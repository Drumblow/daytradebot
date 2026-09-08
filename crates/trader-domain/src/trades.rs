use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::Direction;

/// Posição aberta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub id: Option<i64>,
    pub symbol: String,
    pub signal_id: i64,
    pub direction: Direction,
    pub quantity: Decimal,
    pub avg_entry_price: Decimal,
    pub entry_time: DateTime<Utc>,
    pub stop_price: Decimal,
    pub target_price: Option<Decimal>,
    pub unrealized_pnl: Decimal,
    pub realized_pnl: Decimal,
    pub exit_price: Option<Decimal>,
    pub exit_reason: Option<ExitReason>,
    pub status: PositionStatus,
    pub closed_at: Option<DateTime<Utc>>,
    pub broker: String,
    pub metadata: serde_json::Value,
    pub correlation_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PositionStatus {
    Open,
    Closed,
}

impl Position {
    pub fn new(
        symbol: impl Into<String>,
        signal_id: i64,
        direction: Direction,
        quantity: Decimal,
        avg_entry_price: Decimal,
        stop_price: Decimal,
        broker: impl Into<String>,
    ) -> Result<Self, crate::ValidationError> {
        if quantity <= Decimal::ZERO {
            return Err(crate::ValidationError::InvalidQuantity(
                "quantidade da posição deve ser positiva".to_string(),
            ));
        }
        Ok(Self {
            id: None,
            symbol: symbol.into(),
            signal_id,
            direction,
            quantity,
            avg_entry_price,
            entry_time: Utc::now(),
            stop_price,
            target_price: None,
            unrealized_pnl: Decimal::ZERO,
            realized_pnl: Decimal::ZERO,
            exit_price: None,
            exit_reason: None,
            status: PositionStatus::Open,
            closed_at: None,
            broker: broker.into(),
            metadata: serde_json::Value::Object(Default::default()),
            correlation_id: uuid::Uuid::new_v4().to_string(),
        })
    }
}

/// Trade fechado.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trade {
    pub id: Option<i64>,
    pub symbol: String,
    pub signal_id: i64,
    pub position_id: Option<i64>,
    pub direction: Direction,
    pub entry_price: Decimal,
    pub exit_price: Decimal,
    pub quantity: Decimal,
    pub entry_time: DateTime<Utc>,
    pub exit_time: DateTime<Utc>,
    pub stop_price: Decimal,
    pub target_price: Option<Decimal>,
    pub gross_pnl: Decimal,
    pub commissions: Decimal,
    pub fees: Decimal,
    pub net_pnl: Decimal,
    pub risk_amount: Decimal,
    pub result_in_r: Decimal,
    pub exit_reason: ExitReason,
    pub strategy_id: String,
    pub strategy_version: String,
    pub config_hash: String,
    pub journal: serde_json::Value,
    pub correlation_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExitReason {
    Target,
    Stop,
    Time,
    Manual,
    RiskManager,
    /// Encerramento no fim do pregão (ADR-018).
    ///
    /// O live fecha tudo a mercado entre 15h55 e 16h10 ET porque as pernas do
    /// bracket vão com TIF Day e expiram no sino — uma posição que atravessa a
    /// noite fica sem stop. O backtest replica isso na última barra RTH do dia.
    /// Antes desta variante o live gravava `Manual` com
    /// `journal.forced_exit = "session_flatten"`, e o backtest não fechava nada.
    EndOfDay,
}

impl ExitReason {
    /// Texto canônico da variante — o mesmo do serde, do CHECK de
    /// `trades.exit_reason` e das chaves de métrica.
    ///
    /// Existe para haver UM lugar com essa tabela: antes ela estava duplicada
    /// no repositório (escrita e leitura) e reaparecia em cada relatório.
    pub fn as_str(&self) -> &'static str {
        match self {
            ExitReason::Target => "target",
            ExitReason::Stop => "stop",
            ExitReason::Time => "time",
            ExitReason::Manual => "manual",
            ExitReason::RiskManager => "risk_manager",
            ExitReason::EndOfDay => "end_of_day",
        }
    }
}

impl Trade {
    /// `true` se o trade foi marcado no journal como artefato operacional
    /// (ex.: bug de latência já corrigido) — não deve entrar em métricas de
    /// validação nem no estado de risco reconstruído.
    pub fn is_latency_artifact(&self) -> bool {
        self.journal
            .get("latency_artifact")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    /// Motivo de saída para efeito de métrica, reconhecendo o flatten de fim
    /// de pregão gravado antes do ADR-018.
    ///
    /// Até o ADR-018 não existia `ExitReason::EndOfDay`: o live marcava o
    /// encerramento das 15h55 como `Manual` e deixava a assinatura no journal
    /// (`forced_exit = "session_flatten"`). Quem agrupa por motivo de saída
    /// tem de usar este método, senão os trades anteriores ao ADR entram como
    /// saída discricionária e o gate B mistura categorias.
    ///
    /// A reclassificação das linhas no banco é decisão do dono
    /// (`sql/maintenance/0004-reclassificar-flatten.sql`); com ou sem ela,
    /// este método devolve a mesma resposta.
    pub fn effective_exit_reason(&self) -> ExitReason {
        if self.exit_reason == ExitReason::Manual
            && self.journal.get("forced_exit").and_then(|v| v.as_str()) == Some("session_flatten")
        {
            return ExitReason::EndOfDay;
        }
        self.exit_reason
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O texto serde de cada variante é o mesmo do CHECK de
    /// `trades.exit_reason` no banco (migração 0004 para `end_of_day`). Se
    /// alguém renomear uma variante sem migrar, este teste quebra antes do
    /// INSERT falhar em produção.
    #[test]
    fn exit_reason_serializa_em_snake_case() {
        let casos = [
            (ExitReason::Target, "\"target\""),
            (ExitReason::Stop, "\"stop\""),
            (ExitReason::Time, "\"time\""),
            (ExitReason::Manual, "\"manual\""),
            (ExitReason::RiskManager, "\"risk_manager\""),
            (ExitReason::EndOfDay, "\"end_of_day\""),
        ];
        for (reason, esperado) in casos {
            let json = serde_json::to_string(&reason).expect("serializa");
            assert_eq!(json, esperado, "serde de {reason:?}");
            let volta: ExitReason = serde_json::from_str(&json).expect("desserializa");
            assert_eq!(volta, reason, "round-trip de {reason:?}");
            // `as_str` é a mesma tabela — se divergir, o banco e as métricas
            // passam a falar línguas diferentes.
            assert_eq!(
                format!("\"{}\"", reason.as_str()),
                esperado,
                "as_str de {reason:?}"
            );
        }
    }

    fn trade(exit_reason: ExitReason, journal: serde_json::Value) -> Trade {
        Trade {
            id: None,
            symbol: "IJS".to_string(),
            signal_id: 1,
            position_id: None,
            direction: Direction::Long,
            entry_price: Decimal::from(100),
            exit_price: Decimal::from(101),
            quantity: Decimal::from(10),
            entry_time: Utc::now(),
            exit_time: Utc::now(),
            stop_price: Decimal::from(99),
            target_price: None,
            gross_pnl: Decimal::from(10),
            commissions: Decimal::ZERO,
            fees: Decimal::ZERO,
            net_pnl: Decimal::from(10),
            risk_amount: Decimal::from(10),
            result_in_r: Decimal::ONE,
            exit_reason,
            strategy_id: "s".to_string(),
            strategy_version: "1.0.0".to_string(),
            config_hash: "h".to_string(),
            journal,
            correlation_id: "c".to_string(),
        }
    }

    /// Trades gravados antes do ADR-018 marcavam o flatten como `Manual` com
    /// a assinatura no journal. Agrupar por `exit_reason` cru os contaria como
    /// saída discricionária.
    #[test]
    fn flatten_antigo_e_reconhecido_pelo_journal() {
        let antigo = trade(
            ExitReason::Manual,
            serde_json::json!({ "forced_exit": "session_flatten" }),
        );
        assert_eq!(antigo.effective_exit_reason(), ExitReason::EndOfDay);

        // Saída discricionária de verdade continua `Manual`.
        let manual = trade(ExitReason::Manual, serde_json::json!({}));
        assert_eq!(manual.effective_exit_reason(), ExitReason::Manual);

        // Outro `forced_exit` não vira fim de pregão.
        let outro = trade(
            ExitReason::Manual,
            serde_json::json!({ "forced_exit": "circuit_breaker" }),
        );
        assert_eq!(outro.effective_exit_reason(), ExitReason::Manual);

        // Motivos próprios passam intactos.
        for reason in [
            ExitReason::Target,
            ExitReason::Stop,
            ExitReason::Time,
            ExitReason::RiskManager,
            ExitReason::EndOfDay,
        ] {
            assert_eq!(
                trade(
                    reason,
                    serde_json::json!({ "forced_exit": "session_flatten" })
                )
                .effective_exit_reason(),
                reason,
                "{reason:?} não deve ser reinterpretado"
            );
        }
    }
}

/// Resumo da conta no broker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountSummary {
    pub broker: String,
    pub account_id: Option<String>,
    pub cash: Decimal,
    pub equity: Decimal,
    pub buying_power: Decimal,
    pub daily_pnl: Decimal,
    pub timestamp: DateTime<Utc>,
    /// Moedas que o broker declarou para os valores da conta, fora a linha
    /// "BASE" (ADR-020 §5). Vazio = o broker não informou.
    ///
    /// Existe porque o cap de notional trata `NET_LIQUIDATION` como
    /// **dólares** e ninguém nunca conferiu: se a conta paper for em CAD, o
    /// teto de 1× está ≈ 1,37× errado desde sempre. É uma LISTA, e não um
    /// campo `currency`, porque a conta pode reportar mais de uma — e nesse
    /// caso o certo é mostrar a ambiguidade, não escolher uma e chamá-la de
    /// "a moeda da conta".
    #[serde(default)]
    pub currencies: Vec<String>,
}
