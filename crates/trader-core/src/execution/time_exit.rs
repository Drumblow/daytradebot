//! Saída ativa por tempo (validação pós-entrada em R).
//!
//! Implementa a regra "the trade should be immediately profitable (within one
//! to three bars)" do failure test (Grimes, Cap. 6): se a posição não atingir
//! `min_r` de lucro flutuante dentro de `candles` candles após o fill, ela
//! deve ser encerrada a mercado no fechamento desse candle.
//!
//! A lógica é pura e compartilhada entre o live (`paper.rs`) e o backtest
//! (`BacktestEngine`) — os dois lados apenas alimentam o tracker com o fill
//! da entrada e o fechamento de cada candle, garantindo paridade.

use rust_decimal::Decimal;
use trader_domain::Direction;

/// Configuração da saída por tempo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeExitConfig {
    /// Liga/desliga o mecanismo (desligado por default nas estratégias que
    /// não prescrevem validação rápida, ex.: pullback-trend-v1).
    pub enabled: bool,
    /// Lucro flutuante mínimo, em múltiplos do risco inicial (R), que valida
    /// o trade dentro da janela.
    pub min_r: Decimal,
    /// Candles após o fill dentro dos quais o trade deve se validar.
    pub candles: u32,
}

impl Default for TimeExitConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            min_r: Decimal::from(5) / Decimal::from(10), // 0.5R
            candles: 3,
        }
    }
}

/// Acompanha uma posição aberta e decide se ela deve ser encerrada a mercado
/// no fechamento do candle atual.
///
/// Uma vez que o lucro flutuante atinge `min_r` em qualquer fechamento dentro
/// da janela, o trade está "validado" e a saída por tempo é desarmada (o
/// trade segue até stop ou alvo).
#[derive(Debug, Clone)]
pub struct TimeExitTracker {
    config: TimeExitConfig,
    tracking: bool,
    entry_price: Decimal,
    risk_per_unit: Decimal,
    direction: Direction,
    candles_since_fill: u32,
    validated: bool,
}

impl TimeExitTracker {
    pub fn new(config: TimeExitConfig) -> Self {
        Self {
            config,
            tracking: false,
            entry_price: Decimal::ZERO,
            risk_per_unit: Decimal::ZERO,
            direction: Direction::Long,
            candles_since_fill: 0,
            validated: false,
        }
    }

    pub fn config(&self) -> TimeExitConfig {
        self.config
    }

    /// `true` enquanto uma posição está sendo acompanhada.
    pub fn is_tracking(&self) -> bool {
        self.tracking
    }

    /// Inicia o acompanhamento de uma posição recém-aberta. Não faz nada se
    /// já houver uma posição sendo rastreada (idempotente por posição).
    pub fn ensure_tracking(
        &mut self,
        entry_price: Decimal,
        stop_price: Decimal,
        direction: Direction,
    ) {
        if self.tracking {
            return;
        }
        self.tracking = true;
        self.entry_price = entry_price;
        self.risk_per_unit = (entry_price - stop_price).abs();
        self.direction = direction;
        self.candles_since_fill = 0;
        self.validated = false;
    }

    /// Para de acompanhar (posição fechada por stop/alvo/manual).
    pub fn reset(&mut self) {
        self.tracking = false;
        self.candles_since_fill = 0;
        self.validated = false;
    }

    /// Processa o fechamento de um candle com posição aberta.
    ///
    /// Retorna `true` quando a posição deve ser encerrada a mercado neste
    /// fechamento: janela de validação esgotada (`candles`) sem ter atingido
    /// `min_r` de lucro flutuante em nenhum fechamento.
    pub fn on_candle_close(&mut self, close: Decimal) -> bool {
        if !self.config.enabled || !self.tracking {
            return false;
        }

        self.candles_since_fill += 1;

        if !self.risk_per_unit.is_zero() {
            let floating = match self.direction {
                Direction::Long => close - self.entry_price,
                Direction::Short => self.entry_price - close,
            };
            if floating / self.risk_per_unit >= self.config.min_r {
                self.validated = true;
            }
        }

        if self.validated {
            return false;
        }

        self.candles_since_fill >= self.config.candles
    }
}

impl Default for TimeExitTracker {
    /// Tracker inerte (config desabilitada) — usado quando a estratégia não
    /// habilita a saída por tempo.
    fn default() -> Self {
        Self::new(TimeExitConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> TimeExitConfig {
        TimeExitConfig {
            enabled: true,
            min_r: Decimal::from(5) / Decimal::from(10),
            candles: 3,
        }
    }

    fn tracker() -> TimeExitTracker {
        let mut t = TimeExitTracker::new(config());
        // entrada 100, stop 99 → risco de 1 ponto por unidade
        t.ensure_tracking(Decimal::from(100), Decimal::from(99), Direction::Long);
        t
    }

    #[test]
    fn exits_at_third_candle_without_validation() {
        // CASO 10 do doc: 3 candles laterais com lucro < 0.5R → saída a
        // mercado no fechamento do 3º candle.
        let mut t = tracker();
        assert!(!t.on_candle_close(Decimal::new(1001, 1))); // +0.1R
        assert!(!t.on_candle_close(Decimal::new(1002, 1))); // +0.2R
        assert!(t.on_candle_close(Decimal::new(1001, 1))); // 3º candle, < 0.5R
    }

    #[test]
    fn validation_at_second_candle_disarms_time_exit() {
        // Variante do CASO 10: lucro ≥ 0.5R no 2º candle → posição mantida.
        let mut t = tracker();
        assert!(!t.on_candle_close(Decimal::new(1002, 1)));
        assert!(!t.on_candle_close(Decimal::new(1006, 1))); // +0.6R valida
                                                            // Mesmo caindo abaixo de 0.5R depois, a saída por tempo não dispara.
        assert!(!t.on_candle_close(Decimal::new(1001, 1)));
        assert!(!t.on_candle_close(Decimal::new(1000, 1)));
    }

    #[test]
    fn disabled_config_never_exits() {
        let mut t = TimeExitTracker::new(TimeExitConfig::default());
        t.ensure_tracking(Decimal::from(100), Decimal::from(99), Direction::Long);
        for _ in 0..10 {
            assert!(!t.on_candle_close(Decimal::from(99)));
        }
    }

    #[test]
    fn reset_stops_tracking() {
        let mut t = tracker();
        t.reset();
        assert!(!t.is_tracking());
        assert!(!t.on_candle_close(Decimal::from(99)));
    }

    #[test]
    fn ensure_tracking_is_idempotent() {
        let mut t = tracker();
        t.on_candle_close(Decimal::new(1001, 1));
        // Um segundo fill lógico não deve reiniciar a contagem da posição.
        t.ensure_tracking(Decimal::from(200), Decimal::from(190), Direction::Long);
        assert!(!t.on_candle_close(Decimal::new(1001, 1)));
        assert!(t.on_candle_close(Decimal::new(1001, 1))); // 3º candle da 1ª posição
    }

    #[test]
    fn short_direction_computes_floating_r() {
        let mut t = TimeExitTracker::new(config());
        t.ensure_tracking(Decimal::from(100), Decimal::from(101), Direction::Short);
        // Queda de 0.6 ponto = +0.6R para short → valida no 1º candle.
        assert!(!t.on_candle_close(Decimal::new(994, 1)));
        assert!(!t.on_candle_close(Decimal::from(100)));
        assert!(!t.on_candle_close(Decimal::from(100)));
    }
}
