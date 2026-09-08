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
        CommissionModel::PerTrade(rust_decimal::Decimal::from(35) / rust_decimal::Decimal::from(100))
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
