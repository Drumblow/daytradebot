//! Sessão de negociação em horário de Nova York.
//!
//! As janelas de negociação eram tuplas UTC FIXAS calibradas para o horário
//! de verão americano. Na volta ao EST (1º/nov/2026) elas deslizam uma hora:
//! a janela da `opening-reversal-v1` viraria 08h30–09h30 ET e a estratégia
//! morreria em silêncio (a janela termina no sino), e as demais perderiam a
//! última hora de pregão (A2 da auditoria de 30/08/2026).
//!
//! Aqui as janelas são declaradas em horário de NY e convertidas com
//! `chrono-tz`, que já sabe as regras do DST. Este módulo é a ÚNICA
//! implementação da regra: o `RiskManager` e as janelas próprias das
//! estratégias chamam as mesmas funções.

use chrono::{DateTime, NaiveTime, Timelike, Utc};
use chrono_tz::America::New_York;

/// Hora local de Nova York do instante informado.
pub fn et_time(ts: DateTime<Utc>) -> NaiveTime {
    ts.with_timezone(&New_York).time()
}

/// Faz parse de `"HH:MM:SS"` (horário de NY) para `NaiveTime`.
///
/// Componente inválido vira zero — comportamento herdado das estratégias,
/// preservado para não mudar duas coisas de uma vez.
pub fn parse_et_time(raw: &str) -> NaiveTime {
    let parts: Vec<&str> = raw.split(':').collect();
    NaiveTime::from_hms_opt(
        parts.first().and_then(|v| v.parse().ok()).unwrap_or(0),
        parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(0),
        parts.get(2).and_then(|v| v.parse().ok()).unwrap_or(0),
    )
    .unwrap_or_default()
}

/// `true` quando o instante cai dentro de `[start, end]` em horário de NY.
/// As tuplas são `(hora, minuto, segundo)` de Nova York, inclusivas nas duas
/// pontas.
pub fn within_trading_window(
    ts: DateTime<Utc>,
    start: (u32, u32, u32),
    end: (u32, u32, u32),
) -> bool {
    let seconds = |h: u32, m: u32, s: u32| h * 3600 + m * 60 + s;
    let time = et_time(ts);
    let current = seconds(time.hour(), time.minute(), time.second());
    current >= seconds(start.0, start.1, start.2) && current <= seconds(end.0, end.1, end.2)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    /// A mesma janela de NY vale nos dois lados da virada do DST — o ponto
    /// inteiro do A2. Janela 09h45–15h30 ET.
    #[test]
    fn janela_de_ny_sobrevive_a_virada_do_dst() {
        let start = (9, 45, 0);
        let end = (15, 30, 0);

        // Verão (EDT, UTC-4): 09h45 ET = 13h45 UTC.
        assert!(!within_trading_window(
            utc("2026-08-31T13:44:00Z"),
            start,
            end
        ));
        assert!(within_trading_window(
            utc("2026-08-31T13:45:00Z"),
            start,
            end
        ));
        assert!(within_trading_window(
            utc("2026-08-31T19:30:00Z"),
            start,
            end
        ));
        assert!(!within_trading_window(
            utc("2026-08-31T19:31:00Z"),
            start,
            end
        ));

        // Inverno (EST, UTC-5): 09h45 ET = 14h45 UTC. Com a tupla UTC fixa
        // antiga, 14h45 UTC caía FORA da janela 13h45–19h30.
        assert!(!within_trading_window(
            utc("2026-11-02T14:44:00Z"),
            start,
            end
        ));
        assert!(within_trading_window(
            utc("2026-11-02T14:45:00Z"),
            start,
            end
        ));
        assert!(within_trading_window(
            utc("2026-11-02T20:30:00Z"),
            start,
            end
        ));
        assert!(!within_trading_window(
            utc("2026-11-02T20:31:00Z"),
            start,
            end
        ));
    }

    /// A janela da opening-reversal (09h30–10h30 ET) é a que MORRIA no
    /// inverno: em UTC fixo ela viraria 08h30–09h30 ET, terminando no sino.
    #[test]
    fn janela_da_primeira_hora_continua_valendo_no_inverno() {
        let start = (9, 30, 0);
        let end = (10, 30, 0);
        // 10h00 ET no inverno = 15h00 UTC.
        assert!(within_trading_window(
            utc("2026-11-02T15:00:00Z"),
            start,
            end
        ));
        // 10h00 ET no verão = 14h00 UTC.
        assert!(within_trading_window(
            utc("2026-08-31T14:00:00Z"),
            start,
            end
        ));
    }

    #[test]
    fn parse_de_horario_invalido_vira_zero() {
        assert_eq!(
            parse_et_time("09:45:00"),
            NaiveTime::from_hms_opt(9, 45, 0).unwrap()
        );
        assert_eq!(
            parse_et_time("invalido"),
            NaiveTime::from_hms_opt(0, 0, 0).unwrap()
        );
    }
}
