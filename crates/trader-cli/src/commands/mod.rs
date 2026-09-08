//! Comandos do `trader-cli`.

use trader_infra::config::SessionSettings;

/// Horário do flatten de fim de pregão para o motor de backtest (ADR-018).
///
/// Vem do `[session]` da config — a mesma seção de onde o live tira a janela
/// de encerramento a mercado. `--no-flatten` devolve `None` e reproduz os
/// runs anteriores ao ADR-018 (413–421); é assim que se mede o delta por par.
pub fn session_flatten_et(session: &SessionSettings, no_flatten: bool) -> Option<(u32, u32)> {
    if no_flatten {
        return None;
    }
    use chrono::Timelike;
    let t = trader_core::session::parse_et_time(&session.last_bar);
    Some((t.hour(), t.minute()))
}

/// Modelo de custo de execução do simulador.
///
/// `legacy = true` restaura o mundo anterior ao §5.6 do plano: comissão de
/// US$ 0,35 fixos por perna e alvo limite que enche sem pagar nada. Serve
/// para REPRODUZIR os runs de até 08/09/2026 — nenhuma corretora cobra
/// assim, e a diferença é de uma ordem de grandeza nos tamanhos operados.
pub fn commission_model(legacy: bool) -> trader_adapters::simulated::CommissionModel {
    use trader_adapters::simulated::CommissionModel;
    if legacy {
        CommissionModel::PerTrade(
            rust_decimal::Decimal::from(35) / rust_decimal::Decimal::from(100),
        )
    } else {
        CommissionModel::ibkr_fixed_us()
    }
}

/// Desconto no fill do alvo, como fração do preço.
///
/// Zero no modo legado (o alvo enchia de graça). `override_bps` existe porque
/// este é o parâmetro **escolhido a mão** do modelo de custo — e ele responde
/// por 56% da queda de P&L entre a régua antiga e a nova, mais que a comissão.
/// Um número dessa importância tem de ser variável para ser criticável.
pub fn limit_fill_haircut(
    legacy: bool,
    override_bps: Option<rust_decimal::Decimal>,
) -> rust_decimal::Decimal {
    if let Some(bps) = override_bps {
        return bps / rust_decimal::Decimal::from(10_000);
    }
    if legacy {
        rust_decimal::Decimal::ZERO
    } else {
        rust_decimal::Decimal::from(2) / rust_decimal::Decimal::from(10_000)
    }
}

/// A régua de custo da PRODUÇÃO, como ela aparece gravada em `metrics`.
///
/// É o que o `analyze` exige do baseline do gate B. Espelha, do lado Rust, o
/// `exige_mesma_regua` do `trader-research`: comparar o paper com um backtest
/// medido a outro custo dá um veredito que não descreve nem um nem outro.
pub fn regua_de_producao() -> (&'static str, String, String) {
    use trader_adapters::simulated::SimulatedBrokerConfig;
    let padrao = SimulatedBrokerConfig::default();
    let bps = |v: rust_decimal::Decimal| {
        (v * rust_decimal::Decimal::from(10_000))
            .normalize()
            .to_string()
    };
    (
        commission_label(false),
        bps(padrao.slippage_pct),
        bps(padrao.limit_fill_haircut_pct),
    )
}

/// Rótulo do modelo de custo, para gravar em `metrics` (ADR-019 §3).
///
/// Sem isto, um run com a comissão real e outro com a antiga ficam
/// indistinguíveis no banco e o `analyze` pode pegar o errado como baseline
/// do gate B — o mesmo defeito que o `slippage_bps` e o `session_flatten`
/// fecharam.
pub fn commission_label(legacy: bool) -> &'static str {
    if legacy {
        "fixed-0.35"
    } else {
        "ibkr-fixed-us"
    }
}

/// Overrides de dimensionamento para o harness (ADR-020 §6).
///
/// Os modos A/B/B'/C do plano §5.5 são combinações destes campos. Qualquer um
/// deles preenchido torna o run **experimental**: PF e avg R são quase
/// invariantes ao tamanho, mas P&L em $, DD e a fração de trades presos no
/// cap não são — e um run de modo não pode virar baseline do gate B por
/// ordem de chegada, que é o defeito que o ADR-019 §3 fechou para `--set` e
/// o §5.6 teve de fechar de novo para o custo e o slippage.
#[derive(Debug, Clone, Default)]
pub struct SizingOverrides {
    pub risk_pct: Option<rust_decimal::Decimal>,
    pub capital_fraction: Option<rust_decimal::Decimal>,
    pub notional_multiple: Option<rust_decimal::Decimal>,
    pub notional_usd: Option<rust_decimal::Decimal>,
    pub liquidity_pct: Option<rust_decimal::Decimal>,
}

impl SizingOverrides {
    /// Nenhum override: o run usa o `[risk]` da config, como produção.
    pub fn vazio(&self) -> bool {
        self.risk_pct.is_none()
            && self.capital_fraction.is_none()
            && self.notional_multiple.is_none()
            && self.notional_usd.is_none()
            && self.liquidity_pct.is_none()
    }

    /// Aplica os overrides sobre o `[risk]` da aplicação.
    ///
    /// A validação de faixa NÃO acontece aqui: acontece no
    /// `build_risk_config`, que é por onde live, backtest e walk-forward
    /// passam. Duplicá-la abriria a porta para as duas divergirem.
    pub fn aplica(
        &self,
        risk: &trader_infra::config::RiskSettings,
    ) -> anyhow::Result<trader_infra::config::RiskSettings> {
        use rust_decimal::prelude::ToPrimitive;

        let f64_de = |nome: &str, v: rust_decimal::Decimal| -> anyhow::Result<f64> {
            v.to_f64()
                .ok_or_else(|| anyhow::anyhow!("--{nome} {v} não cabe em f64"))
        };

        let mut saida = risk.clone();
        if let Some(v) = self.risk_pct {
            saida.risk_per_trade_pct = f64_de("risk-pct", v)?;
        }
        if let Some(v) = self.capital_fraction {
            saida.capital_fraction = f64_de("capital-fraction", v)?;
        }
        if let Some(v) = self.notional_multiple {
            saida.max_notional_multiple = f64_de("notional-multiple", v)?;
        }
        if let Some(v) = self.notional_usd {
            saida.max_notional_usd = Some(f64_de("notional-usd", v)?);
        }
        if let Some(v) = self.liquidity_pct {
            saida.max_pct_of_median_bar_notional = Some(f64_de("liquidity-pct", v)?);
        }
        Ok(saida)
    }

    /// O que foi sobrescrito, para imprimir e para gravar no `metrics`.
    pub fn descricao(&self) -> Vec<(String, String)> {
        let mut saida = Vec::new();
        let mut push = |k: &str, v: Option<rust_decimal::Decimal>| {
            if let Some(v) = v {
                saida.push((k.to_string(), v.normalize().to_string()));
            }
        };
        push("risk_per_trade_pct", self.risk_pct);
        push("capital_fraction", self.capital_fraction);
        push("max_notional_multiple", self.notional_multiple);
        push("max_notional_usd", self.notional_usd);
        push("max_pct_of_median_bar_notional", self.liquidity_pct);
        saida
    }
}

/// O dimensionamento com que um run foi medido, para gravar em `metrics`.
///
/// Sem isto, dois runs de modos diferentes ficam indistinguíveis no banco —
/// o mesmo defeito que o `commission_model` fechou para o custo. E o
/// `capital_fraction` é obrigatório para ler o `max_drawdown_pct`: ele é a
/// BASE do DD, e um run fracionado tem DD% comparável só contra outro que
/// declare a mesma base.
pub fn sizing_json(risk: &trader_core::risk::RiskConfig) -> serde_json::Value {
    serde_json::json!({
        "risk_per_trade_pct": risk.risk_per_trade_pct.normalize().to_string(),
        "capital_fraction": risk.capital_fraction.normalize().to_string(),
        "max_notional_multiple": risk.max_notional_multiple.normalize().to_string(),
        "max_notional_usd": risk.max_notional_usd.map(|v| v.normalize().to_string()),
        "max_pct_of_median_bar_notional": risk
            .max_pct_of_median_bar_notional
            .map(|v| v.normalize().to_string()),
        "liquidity_lookback_bars": risk.liquidity_lookback_bars,
    })
}

pub mod account;
pub mod analyze;
pub mod backtest;
pub mod debug_candles;
pub mod flatten;
pub mod ingest;
pub mod journal;
pub mod paper;
pub mod status;
pub mod test_connection;
pub mod walkforward;
