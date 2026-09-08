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
