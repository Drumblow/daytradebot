//! `latest_for` só devolve baseline medido com a MESMA régua.
//!
//! O ADR-019 §3 fechou o caso "a primeira ablação vira baseline do gate B em
//! silêncio" filtrando por par, `config_hash` e `experimental`. O §5.6 abriu o
//! mesmo buraco por outra porta: em 08/09/2026 o simulador passou a cobrar a
//! tabela por ação da IBKR (**9,9×** o modelo fixo antigo, medido sobre os 214
//! trades OOS) e o banco passou a ter os dois custos para o mesmo trio —
//! inclusive runs `--legacy-cost` feitos para o teste de paridade, que são os
//! MAIS RECENTES.
//!
//! Sem o filtro, o gate B compara um paper que paga comissão real contra um
//! backtest que paga um décimo dela, e o live parece pior por um motivo que
//! não é a estratégia. O mesmo vale para os outros dois eixos da régua —
//! slippage e desconto no alvo —, que também não marcam o run como
//! experimental. Estes testes travam os três.

use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::PgPool;

use trader_infra::repositories::{BacktestRunRecord, SqlxBacktestRunRepository};

const ESTRATEGIA: &str = "balance-area-breakout-v1";
const PAR: &str = "IJS";
const HASH: &str = "be598d6d4b518b92";

fn record(final_equity: i64, metrics: serde_json::Value, label: &str) -> BacktestRunRecord {
    BacktestRunRecord {
        symbol: PAR.to_string(),
        strategy_id: ESTRATEGIA.to_string(),
        strategy_version: "1.0.0".to_string(),
        config_hash: HASH.to_string(),
        timeframe: "M15".to_string(),
        period_start: Utc::now() - chrono::Duration::days(400),
        period_end: Utc::now(),
        initial_capital: Decimal::from(100_000),
        final_equity: Decimal::from(final_equity),
        metrics,
        label: Some(label.to_string()),
    }
}

fn metrics(commission_model: Option<&str>, experimental: bool) -> serde_json::Value {
    metrics_com(commission_model, experimental, "2", "2")
}

fn metrics_com(
    commission_model: Option<&str>,
    experimental: bool,
    slippage_bps: &str,
    haircut_bps: &str,
) -> serde_json::Value {
    let mut m = serde_json::json!({
        "slippage_bps": slippage_bps,
        "limit_fill_haircut_bps": haircut_bps,
        "session_flatten": "15:45",
        "experimental": experimental,
    });
    if let Some(cm) = commission_model {
        m["commission_model"] = cm.into();
    }
    m
}

/// A régua de produção, como o `analyze` a monta.
const REGUA: (&str, &str, &str) = ("ibkr-fixed-us", "2", "2");

#[sqlx::test(migrations = "src/db/migrations")]
async fn ignora_o_run_mais_recente_se_o_custo_for_outro(pool: PgPool) {
    let repo = SqlxBacktestRunRepository::new(pool);

    // Primeiro o run com o custo real...
    repo.save(&record(
        102_996,
        metrics(Some("ibkr-fixed-us"), false),
        "gate-a-custo-real",
    ))
    .await
    .expect("salvar run com custo real");

    // ...e DEPOIS o de paridade, com o custo antigo. É o mais recente.
    repo.save(&record(
        103_384,
        metrics_com(Some("fixed-0.35"), false, "2", "0"),
        "paridade",
    ))
    .await
    .expect("salvar run legado");

    let baseline = repo
        .latest_for(ESTRATEGIA, PAR, HASH, REGUA.0, REGUA.1, REGUA.2)
        .await
        .expect("consulta")
        .expect("deve achar o run com o custo pedido");

    // Sem o filtro, viria o de 103.384 — o legado, mais recente.
    assert_eq!(baseline.final_equity, Decimal::from(102_996));
    assert_eq!(baseline.label.as_deref(), Some("gate-a-custo-real"));
}

#[sqlx::test(migrations = "src/db/migrations")]
async fn run_sem_modelo_de_comissao_nao_serve_de_baseline(pool: PgPool) {
    let repo = SqlxBacktestRunRepository::new(pool);

    // Todo run anterior a 08/09/2026 é assim: não declara o custo.
    repo.save(&record(103_384, metrics(None, false), "gate-a-adr019"))
        .await
        .expect("salvar run antigo");

    let baseline = repo
        .latest_for(ESTRATEGIA, PAR, HASH, REGUA.0, REGUA.1, REGUA.2)
        .await
        .expect("consulta");

    // Falha fechado: melhor avisar do que comparar com a régua errada.
    assert!(
        baseline.is_none(),
        "run sem `commission_model` não pode casar com nenhum modelo"
    );
}

#[sqlx::test(migrations = "src/db/migrations")]
async fn run_experimental_continua_fora_mesmo_com_o_custo_certo(pool: PgPool) {
    let repo = SqlxBacktestRunRepository::new(pool);

    repo.save(&record(
        102_996,
        metrics(Some("ibkr-fixed-us"), false),
        "gate-a-custo-real",
    ))
    .await
    .expect("salvar run de produção");

    repo.save(&record(
        109_999,
        metrics(Some("ibkr-fixed-us"), true),
        "ablacao-alvo-3r",
    ))
    .await
    .expect("salvar ablação");

    let baseline = repo
        .latest_for(ESTRATEGIA, PAR, HASH, REGUA.0, REGUA.1, REGUA.2)
        .await
        .expect("consulta")
        .expect("deve achar o run de produção");

    assert_eq!(baseline.final_equity, Decimal::from(102_996));
}

#[sqlx::test(migrations = "src/db/migrations")]
async fn o_custo_legado_ainda_e_recuperavel_quando_pedido(pool: PgPool) {
    // O filtro não pode tornar o run de paridade inalcançável: quem quiser
    // comparar as duas réguas precisa dos dois lados.
    let repo = SqlxBacktestRunRepository::new(pool);

    repo.save(&record(
        102_996,
        metrics(Some("ibkr-fixed-us"), false),
        "gate-a-custo-real",
    ))
    .await
    .expect("salvar run com custo real");
    repo.save(&record(
        103_384,
        metrics_com(Some("fixed-0.35"), false, "2", "0"),
        "paridade",
    ))
    .await
    .expect("salvar run legado");

    let legado = repo
        .latest_for(ESTRATEGIA, PAR, HASH, "fixed-0.35", "2", "0")
        .await
        .expect("consulta")
        .expect("deve achar o run legado");

    assert_eq!(legado.final_equity, Decimal::from(103_384));
}


#[sqlx::test(migrations = "src/db/migrations")]
async fn run_de_sensibilidade_a_4bp_nao_vira_baseline(pool: PgPool) {
    // O ADR-018 manda rodar `--slippage-bps 4` em IJS e SLYV. Esse run entra
    // no banco NAO-experimental (`--slippage-bps` nunca marcou experimental) e
    // com `created_at` mais novo. Sem o filtro de slippage ele viraria o
    // baseline do gate B da producao, que roda a 2 bp — e 2 bp a mais derrubam
    // o avg R de IJS de 0,305 para 0,160, deslocamento maior que a banda
    // inteira de +-30% do gate.
    let repo = SqlxBacktestRunRepository::new(pool);

    repo.save(&record(
        102_996,
        metrics(Some("ibkr-fixed-us"), false),
        "gate-a-custo-real",
    ))
    .await
    .expect("salvar run de produção");

    repo.save(&record(
        102_266,
        metrics_com(Some("ibkr-fixed-us"), false, "4", "2"),
        "sensibilidade-4bp",
    ))
    .await
    .expect("salvar run de sensibilidade");

    let baseline = repo
        .latest_for(ESTRATEGIA, PAR, HASH, REGUA.0, REGUA.1, REGUA.2)
        .await
        .expect("consulta")
        .expect("deve achar o run de produção");

    assert_eq!(baseline.final_equity, Decimal::from(102_996));
    assert_eq!(baseline.label.as_deref(), Some("gate-a-custo-real"));
}

#[sqlx::test(migrations = "src/db/migrations")]
async fn run_com_outro_desconto_no_alvo_nao_vira_baseline(pool: PgPool) {
    // Mesmo raciocinio para o `--limit-haircut-bps`: ele existe justamente
    // para variar o parametro escolhido a mao, e um run de sensibilidade nao
    // pode virar a referencia do gate B.
    let repo = SqlxBacktestRunRepository::new(pool);

    repo.save(&record(
        102_996,
        metrics(Some("ibkr-fixed-us"), false),
        "gate-a-custo-real",
    ))
    .await
    .expect("salvar run de produção");

    repo.save(&record(
        103_200,
        metrics_com(Some("ibkr-fixed-us"), false, "2", "0"),
        "sensibilidade-haircut-0",
    ))
    .await
    .expect("salvar run sem haircut");

    let baseline = repo
        .latest_for(ESTRATEGIA, PAR, HASH, REGUA.0, REGUA.1, REGUA.2)
        .await
        .expect("consulta")
        .expect("deve achar o run de produção");

    assert_eq!(baseline.final_equity, Decimal::from(102_996));
}
