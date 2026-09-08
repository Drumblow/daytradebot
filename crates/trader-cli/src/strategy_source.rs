//! De onde vem a config da estratégia num run: o TOML canônico, um TOML
//! alternativo (`--strategy-config`) ou o canônico com sobrescritas
//! (`--set chave=valor`). ADR-019 §1 e §2.
//!
//! Existe porque, até aqui, cada variação de parâmetro exigia um módulo novo
//! em `trader-core` e um braço em `dispatch.rs` — o que fazia toda ablação
//! custar uma semana e, na prática, fazia ninguém rodar ablação nenhuma.
//!
//! Duas travas, porque a barateza é o risco:
//!
//! 1. **Chave inexistente falha.** Os 9 `StrategyParameters` ganharam
//!    `#[serde(deny_unknown_fields)]`; além disso, se o `config_hash`
//!    resultante for igual ao do TOML sem override, o run aborta — sinal de
//!    que o `--set` não mudou nada e o rótulo mentiria.
//! 2. **O id do TOML tem de bater com o `--strategy`.** `load_strategy` casa
//!    o id pela string do argumento, não pelo conteúdo do arquivo: sem esta
//!    checagem, `--strategy-config` de outra estratégia rodaria calado.

use anyhow::{bail, Context, Result};

/// Config de estratégia resolvida para um run.
#[derive(Debug)]
pub struct ResolvedStrategy {
    /// TOML final (já com os overrides aplicados).
    pub toml: String,
    /// De onde veio, para log e para o `metrics` do run.
    pub source: String,
    /// Sobrescritas aplicadas, na ordem em que vieram.
    pub overrides: Vec<(String, String)>,
    /// `true` quando o run não usa a config canônica de produção.
    pub is_experimental: bool,
}

/// Caminho canônico da config de uma estratégia.
pub fn canonical_path(strategy_id: &str) -> String {
    format!("config/strategies/{strategy_id}.toml")
}

/// Resolve a config de estratégia de um run.
///
/// `sets` são pares `chave=valor` aplicados em `[strategy.parameters]`. O
/// valor é interpretado como TOML (`3`, `true`, `1.5`); o que não for escalar
/// TOML válido vira string, que é o que faz `trading_end_time=11:45:00`
/// funcionar.
pub fn resolve(
    strategy_id: &str,
    strategy_config: Option<&str>,
    sets: &[String],
) -> Result<ResolvedStrategy> {
    let path = strategy_config
        .map(|p| p.to_string())
        .unwrap_or_else(|| canonical_path(strategy_id));

    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("falha ao ler config da estratégia em {path}"))?;

    // O id do arquivo tem de bater com o `--strategy`: `load_strategy` casa
    // pelo argumento, então um TOML de outra estratégia rodaria calado, com o
    // parse caindo em campos coincidentes.
    let parsed: toml::Value =
        toml::from_str(&raw).with_context(|| format!("TOML inválido em {path}"))?;
    let id_no_arquivo = parsed
        .get("strategy")
        .and_then(|s| s.get("id"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("{path} não tem `strategy.id`"))?;
    if id_no_arquivo != strategy_id {
        bail!(
            "{path} declara strategy.id = \"{id_no_arquivo}\", mas o comando pediu \
             \"{strategy_id}\". O dispatch casa pelo argumento, então isto rodaria \
             a estratégia errada em silêncio."
        );
    }

    if sets.is_empty() {
        return Ok(ResolvedStrategy {
            toml: raw,
            source: path.clone(),
            overrides: Vec::new(),
            is_experimental: strategy_config.is_some(),
        });
    }

    let mut doc = parsed;
    let overrides = apply_sets(&mut doc, sets, &path)?;

    Ok(ResolvedStrategy {
        toml: toml::to_string(&doc).context("falha ao reserializar o TOML com overrides")?,
        source: format!("{path} + --set"),
        overrides,
        is_experimental: true,
    })
}

/// Interpreta o lado direito de um `--set` como valor TOML.
///
/// Aceita apenas inteiro, float, booleano e string entre aspas. **Tudo o mais
/// vira string**, e isso é deliberado: TOML tem literal de hora, então
/// `x = 11:45:00` faz parse como `Datetime` — e todos os campos de horário das
/// estratégias são `String` (`"11:45:00"`), de modo que o valor tipado
/// quebraria o parse com uma mensagem incompreensível. `trading_end_time` é
/// justamente o override que as v2 de janela horária (§6.10) vão usar.
fn parse_toml_scalar(raw: &str) -> toml::Value {
    match toml::from_str::<toml::Value>(&format!("x = {raw}")) {
        Ok(v) => match v.get("x") {
            Some(
                valor @ (toml::Value::Integer(_)
                | toml::Value::Float(_)
                | toml::Value::Boolean(_)
                | toml::Value::String(_)),
            ) => valor.clone(),
            // Datetime, array, tabela: não há campo assim nos
            // `StrategyParameters`; tratar como texto é o comportamento útil.
            _ => toml::Value::String(raw.to_string()),
        },
        Err(_) => toml::Value::String(raw.to_string()),
    }
}

/// Aplica os pares `chave=valor` em `[strategy.parameters]` de um documento
/// TOML já parseado. Separada de `resolve` para poder ser testada sem tocar no
/// disco.
fn apply_sets(
    doc: &mut toml::Value,
    sets: &[String],
    origem: &str,
) -> Result<Vec<(String, String)>> {
    let mut overrides = Vec::new();
    for entry in sets {
        let (chave, valor) = entry
            .split_once('=')
            .ok_or_else(|| anyhow::anyhow!("--set espera `chave=valor`, veio `{entry}`"))?;
        let chave = chave.trim();
        let valor = valor.trim();
        if chave.is_empty() {
            bail!("--set com chave vazia em `{entry}`");
        }

        let params = doc
            .get_mut("strategy")
            .and_then(|s| s.get_mut("parameters"))
            .and_then(|p| p.as_table_mut())
            .ok_or_else(|| anyhow::anyhow!("{origem} não tem `[strategy.parameters]`"))?;

        params.insert(chave.to_string(), parse_toml_scalar(valor));
        overrides.push((chave.to_string(), valor.to_string()));
    }
    Ok(overrides)
}

/// `config_hash` da config canônica da estratégia, para comparar com o hash
/// de um run com override.
///
/// Se os dois forem iguais, o `--set` não mudou nada: ou a chave não existe
/// (e o `deny_unknown_fields` deveria ter pego), ou o valor é o mesmo do
/// arquivo. Nos dois casos o run seria rotulado como ablação sem ser uma.
pub fn canonical_config_hash(strategy_id: &str) -> Result<String> {
    let path = canonical_path(strategy_id);
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("falha ao ler config canônica em {path}"))?;
    let strategy = crate::dispatch::load_strategy(strategy_id, &raw)?;
    Ok(strategy.config_hash())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_tipa_o_valor_como_toml_quando_da() {
        assert_eq!(parse_toml_scalar("3"), toml::Value::Integer(3));
        assert_eq!(parse_toml_scalar("true"), toml::Value::Boolean(true));
        assert_eq!(parse_toml_scalar("1.5"), toml::Value::Float(1.5));
        assert_eq!(
            parse_toml_scalar("\"texto\""),
            toml::Value::String("texto".to_string())
        );
    }

    /// Horário não é escalar TOML válido sem aspas — e é exatamente o tipo de
    /// override que as v2 de janela horária (§6.10) vão precisar.
    #[test]
    fn set_com_horario_vira_string() {
        assert_eq!(
            parse_toml_scalar("11:45:00"),
            toml::Value::String("11:45:00".to_string())
        );
    }

    fn doc_minimo() -> toml::Value {
        toml::from_str(
            "[strategy]
id = \"x\"
[strategy.parameters]
reward_multiple = 2
",
        )
        .expect("TOML de teste")
    }

    #[test]
    fn set_sem_igual_e_erro() {
        let mut doc = doc_minimo();
        let e =
            apply_sets(&mut doc, &["semigual".to_string()], "teste").expect_err("deveria falhar");
        assert!(e.to_string().contains("chave=valor"), "{e}");
    }

    #[test]
    fn set_sobrescreve_a_chave_em_strategy_parameters() {
        let mut doc = doc_minimo();
        let ov = apply_sets(&mut doc, &["reward_multiple=3".to_string()], "teste").unwrap();
        assert_eq!(ov, vec![("reward_multiple".to_string(), "3".to_string())]);
        assert_eq!(
            doc["strategy"]["parameters"]["reward_multiple"],
            toml::Value::Integer(3)
        );
    }

    /// O caso que motiva `parse_toml_scalar` não confiar no parser: TOML tem
    /// literal de hora, e todos os campos de horário das estratégias são
    /// `String`. Se `11:45:00` virasse `Datetime`, o TOML reserializado
    /// deixaria de casar com a struct.
    #[test]
    fn set_de_horario_reserializa_como_string() {
        let mut doc = doc_minimo();
        apply_sets(
            &mut doc,
            &["trading_end_time=11:45:00".to_string()],
            "teste",
        )
        .unwrap();
        let texto = toml::to_string(&doc).unwrap();
        assert!(
            texto.contains("trading_end_time = \"11:45:00\""),
            "esperado string entre aspas, veio:
{texto}"
        );
    }
}
