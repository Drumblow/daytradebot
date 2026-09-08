//! Métricas de performance de backtest.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, MathematicalOps};
use serde::{Deserialize, Serialize};

use trader_domain::Trade;

/// Recorte de métricas sobre um subconjunto de trades.
///
/// Serve a qualquer agrupamento (motivo de saída, direção, hora). O primeiro
/// consumidor é o `end_of_day` do ADR-018: sem ele não dá para responder
/// "quanto do P&L vinha de posição que atravessava a noite?" sem sair do
/// motor para um script.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GroupMetrics {
    pub trades: usize,
    pub wins: usize,
    pub net_pnl: Decimal,
    pub gross_profit: Decimal,
    pub gross_loss: Decimal,
    /// `None` quando não houve perdas no grupo (PF infinito) ou sem trades.
    pub profit_factor: Option<Decimal>,
    pub avg_r: Decimal,
}

impl GroupMetrics {
    fn push(&mut self, trade: &Trade) {
        self.trades += 1;
        self.net_pnl += trade.net_pnl;
        if trade.net_pnl > Decimal::ZERO {
            self.wins += 1;
            self.gross_profit += trade.net_pnl;
        } else {
            self.gross_loss += trade.net_pnl.abs();
        }
        self.avg_r += trade.result_in_r;
    }

    /// Fecha os acumuladores: `avg_r` vira média e o PF é calculado.
    fn finish(&mut self) {
        if self.trades > 0 {
            self.avg_r /= Decimal::from(self.trades as i64);
        }
        self.profit_factor = if self.gross_loss.is_zero() {
            None
        } else {
            Some(self.gross_profit / self.gross_loss)
        };
    }
}

/// Métricas calculadas a partir de uma série de trades.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestMetrics {
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub win_rate: Decimal,
    pub gross_profit: Decimal,
    pub gross_loss: Decimal,
    pub net_pnl: Decimal,
    /// Razão lucro bruto / perda bruta. `None` quando não há perdas no
    /// período (PF matematicamente infinito) ou quando não há trades.
    pub profit_factor: Option<Decimal>,
    pub max_drawdown: Decimal,
    pub max_drawdown_pct: Decimal,
    pub avg_pnl_per_trade: Decimal,
    pub avg_r_per_trade: Decimal,
    pub max_consecutive_losses: usize,
    pub best_trade: Decimal,
    pub worst_trade: Decimal,
    pub sharpe_ratio: Decimal,
    /// Quebra por motivo de saída (ADR-018), chaveada pelo texto canônico de
    /// `ExitReason::as_str`. Vazio quando não há trades.
    ///
    /// Usa `Trade::effective_exit_reason`, então trades gravados antes do
    /// ADR-018 (flatten como `manual` + marca no journal) entram na linha
    /// `end_of_day`, como devem.
    #[serde(default)]
    pub by_exit_reason: BTreeMap<String, GroupMetrics>,
    /// Quebra por direção (`long` / `short`).
    #[serde(default)]
    pub by_direction: BTreeMap<String, GroupMetrics>,
    /// Quebra por hora ET de ENTRADA, chaveada como `"10"`, `"11"`, ...
    ///
    /// A hora tem de ser de Nova York, não UTC: um bucket em UTC muda de
    /// significado na virada do DST e mistura duas horas de pregão diferentes.
    #[serde(default)]
    pub by_entry_hour_et: BTreeMap<String, GroupMetrics>,

    // --- ADR-019 §4: dispersão e concentração ---
    //
    // TODOS com `#[serde(default)]`, sem exceção: o jsonb `metrics` de
    // `backtest_runs` guarda esta struct serializada, e há 700+ runs gravados
    // antes do ADR-019 que não têm nenhum destes campos. Sem o default, o
    // `analyze` aborta com "métricas do run N inválidas" ao cair num deles —
    // ou seja, o histórico inteiro do gate B fica ilegível.
    /// PF calculado em **R**, não em dólares.
    ///
    /// O sizing trava no cap de notional, então o dinheiro arriscado cresce
    /// com a distância do stop; como stop largo ganha, o PF em $ fica acima do
    /// PF em R (balance OOS: 2,09 contra 1,41). O gate lia só o primeiro.
    #[serde(default)]
    pub profit_factor_r: Option<Decimal>,
    /// t-stat do avg R: `avg_R / (desvio_R / sqrt(n))`. Com n < 30 é
    /// indicativo, não teste — mas separa "PF 2 com 8 trades" de "PF 2 com 80".
    #[serde(default)]
    pub t_stat_avg_r: f64,
    /// Correlação entre `risk_amount` e `result_in_r`.
    ///
    /// Mede o quanto o resultado depende do tamanho acidental da posição. Se
    /// for alta, o PF em $ é em boa parte artefato de sizing.
    #[serde(default)]
    pub corr_risk_result: f64,
    /// Pregões distintos com trade.
    #[serde(default)]
    pub trading_days: usize,
    /// Fração do net que veio do melhor dia (0–1). Concentração é o risco
    /// mais subestimado deste projeto: 64% do P&L de uma variante saiu de um
    /// único dia.
    #[serde(default)]
    pub top_day_share: f64,
    /// Fração do net que veio dos 5 melhores dias.
    #[serde(default)]
    pub top5_day_share: f64,
    /// Fração do net que veio dos 2 melhores meses (critério do gate
    /// proposto: ≤ 60%).
    #[serde(default)]
    pub top2_month_share: f64,
    /// Meses com trade e meses com net positivo.
    #[serde(default)]
    pub months_total: usize,
    #[serde(default)]
    pub months_positive: usize,
    /// Comissão + taxas somadas. O slippage não entra: ele já está embutido
    /// nos preços de execução do simulador e não é recuperável do trade.
    #[serde(default)]
    pub cost_total: Decimal,
}

impl BacktestMetrics {
    /// Calcula métricas a partir de uma lista de trades e capital inicial.
    pub fn from_trades(trades: &[Trade], initial_capital: Decimal) -> Self {
        Self::compute(trades, initial_capital, None)
    }

    /// Calcula métricas a partir de um resultado de backtest completo,
    /// incluindo série de equity para Sharpe.
    pub fn from_trades_with_equity(
        trades: &[Trade],
        initial_capital: Decimal,
        daily_equity: &[(DateTime<Utc>, Decimal)],
    ) -> Self {
        Self::compute(trades, initial_capital, Some(daily_equity))
    }

    fn compute(
        trades: &[Trade],
        initial_capital: Decimal,
        daily_equity: Option<&[(DateTime<Utc>, Decimal)]>,
    ) -> Self {
        if trades.is_empty() {
            return Self::empty(initial_capital);
        }

        let total_trades = trades.len();
        let mut winning_trades = 0usize;
        let mut losing_trades = 0usize;
        let mut gross_profit = Decimal::ZERO;
        let mut gross_loss = Decimal::ZERO;
        let mut max_drawdown = Decimal::ZERO;
        let mut max_drawdown_pct = Decimal::ZERO;
        let mut peak = initial_capital;
        let mut current_equity = initial_capital;
        let mut max_consecutive_losses = 0usize;
        let mut current_consecutive_losses = 0usize;
        let mut best_trade = Decimal::MIN;
        let mut worst_trade = Decimal::MAX;
        let mut total_r = Decimal::ZERO;
        let mut by_exit_reason: BTreeMap<String, GroupMetrics> = BTreeMap::new();
        let mut by_direction: BTreeMap<String, GroupMetrics> = BTreeMap::new();
        let mut by_entry_hour_et: BTreeMap<String, GroupMetrics> = BTreeMap::new();
        // R e $ por dia e por mes ET, para as metricas de concentracao.
        let mut por_dia: BTreeMap<chrono::NaiveDate, Decimal> = BTreeMap::new();
        let mut por_mes: BTreeMap<(i32, u32), Decimal> = BTreeMap::new();
        let mut gross_profit_r = Decimal::ZERO;
        let mut gross_loss_r = Decimal::ZERO;
        let mut cost_total = Decimal::ZERO;

        for trade in trades {
            let pnl = trade.net_pnl;
            current_equity += pnl;

            by_exit_reason
                .entry(trade.effective_exit_reason().as_str().to_string())
                .or_default()
                .push(trade);
            by_direction
                .entry(
                    match trade.direction {
                        trader_domain::Direction::Long => "long",
                        trader_domain::Direction::Short => "short",
                    }
                    .to_string(),
                )
                .or_default()
                .push(trade);

            // A hora do bucket e a de NOVA YORK. Em UTC o mesmo bucket muda de
            // significado na virada do DST e junta duas horas de pregao
            // diferentes -- a familia do A2 da auditoria de 30/08.
            let entrada_et = trader_core::session::et_time(trade.entry_time);
            by_entry_hour_et
                .entry(format!("{:02}", chrono::Timelike::hour(&entrada_et)))
                .or_default()
                .push(trade);

            // O dia de referencia e o do FECHAMENTO: e quando o P&L e
            // realizado, e e a granularidade do bootstrap em blocos (§5.3).
            let dia = trader_core::session::et_date(trade.exit_time);
            *por_dia.entry(dia).or_default() += pnl;
            use chrono::Datelike;
            *por_mes.entry((dia.year(), dia.month())).or_default() += pnl;

            if trade.result_in_r > Decimal::ZERO {
                gross_profit_r += trade.result_in_r;
            } else {
                gross_loss_r += trade.result_in_r.abs();
            }
            cost_total += trade.commissions + trade.fees;

            if pnl > Decimal::ZERO {
                winning_trades += 1;
                gross_profit += pnl;
                current_consecutive_losses = 0;
            } else {
                losing_trades += 1;
                gross_loss += pnl.abs();
                current_consecutive_losses += 1;
                max_consecutive_losses = max_consecutive_losses.max(current_consecutive_losses);
            }

            if current_equity > peak {
                peak = current_equity;
            }

            let drawdown = peak - current_equity;
            if drawdown > max_drawdown {
                max_drawdown = drawdown;
                max_drawdown_pct = if peak.is_zero() {
                    Decimal::ZERO
                } else {
                    drawdown / peak * Decimal::from(100)
                };
            }

            best_trade = best_trade.max(pnl);
            worst_trade = worst_trade.min(pnl);
            total_r += trade.result_in_r;
        }

        let net_pnl = gross_profit - gross_loss;
        let win_rate = Decimal::from(winning_trades as i64) / Decimal::from(total_trades as i64)
            * Decimal::from(100);
        let profit_factor = if gross_loss.is_zero() {
            None
        } else {
            Some(gross_profit / gross_loss)
        };
        let avg_pnl_per_trade = net_pnl / Decimal::from(total_trades as i64);
        let avg_r_per_trade = total_r / Decimal::from(total_trades as i64);
        let sharpe_ratio = daily_equity.map(calculate_sharpe).unwrap_or(Decimal::ZERO);
        for group in by_exit_reason
            .values_mut()
            .chain(by_direction.values_mut())
            .chain(by_entry_hour_et.values_mut())
        {
            group.finish();
        }

        let profit_factor_r = if gross_loss_r.is_zero() {
            None
        } else {
            Some(gross_profit_r / gross_loss_r)
        };

        // Estatistica em f64 e permitida (ADR-019 §8): o AGENTS.md proibe f64
        // para DINHEIRO, e nada aqui volta como valor monetario.
        let rs: Vec<f64> = trades
            .iter()
            .filter_map(|t| t.result_in_r.to_f64())
            .collect();
        let t_stat_avg_r = t_stat(&rs);
        let riscos: Vec<f64> = trades
            .iter()
            .filter_map(|t| t.risk_amount.to_f64())
            .collect();
        let corr_risk_result = if riscos.len() == rs.len() {
            correlacao(&riscos, &rs)
        } else {
            0.0
        };

        let net_abs = net_pnl.abs();
        let share = |soma: Decimal| -> f64 {
            if net_abs.is_zero() {
                0.0
            } else {
                (soma / net_abs).to_f64().unwrap_or(0.0)
            }
        };
        let mut dias: Vec<Decimal> = por_dia.values().copied().collect();
        dias.sort_by(|a, b| b.cmp(a));
        let top_day_share = share(dias.first().copied().unwrap_or(Decimal::ZERO));
        let top5_day_share = share(dias.iter().take(5).sum::<Decimal>());

        let mut meses: Vec<Decimal> = por_mes.values().copied().collect();
        meses.sort_by(|a, b| b.cmp(a));
        let top2_month_share = share(meses.iter().take(2).sum::<Decimal>());
        let months_total = meses.len();
        let months_positive = meses.iter().filter(|m| **m > Decimal::ZERO).count();

        Self {
            total_trades,
            winning_trades,
            losing_trades,
            win_rate,
            gross_profit,
            gross_loss,
            net_pnl,
            profit_factor,
            max_drawdown,
            max_drawdown_pct,
            avg_pnl_per_trade,
            avg_r_per_trade,
            max_consecutive_losses,
            best_trade,
            worst_trade,
            sharpe_ratio,
            by_exit_reason,
            by_direction,
            by_entry_hour_et,
            profit_factor_r,
            t_stat_avg_r,
            corr_risk_result,
            trading_days: por_dia.len(),
            top_day_share,
            top5_day_share,
            top2_month_share,
            months_total,
            months_positive,
            cost_total,
        }
    }

    fn empty(_initial_capital: Decimal) -> Self {
        Self {
            total_trades: 0,
            winning_trades: 0,
            losing_trades: 0,
            win_rate: Decimal::ZERO,
            gross_profit: Decimal::ZERO,
            gross_loss: Decimal::ZERO,
            net_pnl: Decimal::ZERO,
            profit_factor: None,
            max_drawdown: Decimal::ZERO,
            max_drawdown_pct: Decimal::ZERO,
            avg_pnl_per_trade: Decimal::ZERO,
            avg_r_per_trade: Decimal::ZERO,
            max_consecutive_losses: 0,
            best_trade: Decimal::ZERO,
            worst_trade: Decimal::ZERO,
            sharpe_ratio: Decimal::ZERO,
            by_exit_reason: BTreeMap::new(),
            by_direction: BTreeMap::new(),
            by_entry_hour_et: BTreeMap::new(),
            profit_factor_r: None,
            t_stat_avg_r: 0.0,
            corr_risk_result: 0.0,
            trading_days: 0,
            top_day_share: 0.0,
            top5_day_share: 0.0,
            top2_month_share: 0.0,
            months_total: 0,
            months_positive: 0,
            cost_total: Decimal::ZERO,
        }
    }

    /// PF para exibição: "∞" quando não houve perdas (mas houve trades),
    /// "N/A" sem trades, ou o valor com 2 casas.
    pub fn profit_factor_display(&self) -> String {
        match self.profit_factor {
            Some(pf) => format!("{pf:.2}"),
            None if self.total_trades > 0 => "∞".to_string(),
            None => "N/A".to_string(),
        }
    }

    /// Idem para o PF em R.
    pub fn profit_factor_r_display(&self) -> String {
        match self.profit_factor_r {
            Some(pf) => format!("{pf:.2}"),
            None if self.total_trades > 0 => "∞".to_string(),
            None => "N/A".to_string(),
        }
    }
}

/// t-stat da media de uma amostra contra zero.
///
/// Devolve 0 com menos de 2 pontos ou desvio nulo -- casos em que a
/// estatistica nao existe, e onde devolver um numero grande seria pior do que
/// devolver nada.
fn t_stat(xs: &[f64]) -> f64 {
    if xs.len() < 2 {
        return 0.0;
    }
    let n = xs.len() as f64;
    let media = xs.iter().sum::<f64>() / n;
    let var = xs.iter().map(|x| (x - media).powi(2)).sum::<f64>() / (n - 1.0);
    if var <= 0.0 {
        return 0.0;
    }
    media / (var.sqrt() / n.sqrt())
}

/// Correlacao de Pearson. 0 quando alguma das series e constante.
fn correlacao(xs: &[f64], ys: &[f64]) -> f64 {
    if xs.len() < 2 || xs.len() != ys.len() {
        return 0.0;
    }
    let n = xs.len() as f64;
    let mx = xs.iter().sum::<f64>() / n;
    let my = ys.iter().sum::<f64>() / n;
    let mut cov = 0.0;
    let mut vx = 0.0;
    let mut vy = 0.0;
    for (x, y) in xs.iter().zip(ys) {
        cov += (x - mx) * (y - my);
        vx += (x - mx).powi(2);
        vy += (y - my).powi(2);
    }
    if vx <= 0.0 || vy <= 0.0 {
        return 0.0;
    }
    cov / (vx.sqrt() * vy.sqrt())
}

/// Calcula um Sharpe simplificado anualizado a partir da série de equity.
///
/// Usa retornos entre amostras consecutivas. Sem taxa livre de risco (rf = 0).
/// A anualização deriva do intervalo mediano entre amostras: para candles de
/// 15min, períodos/ano ≈ 35040; para 1 dia, ≈ 365.
fn calculate_sharpe(equity_series: &[(DateTime<Utc>, Decimal)]) -> Decimal {
    if equity_series.len() < 2 {
        return Decimal::ZERO;
    }

    let returns: Vec<Decimal> = equity_series
        .windows(2)
        .map(|w| {
            let prev = w[0].1;
            let curr = w[1].1;
            if prev.is_zero() {
                Decimal::ZERO
            } else {
                (curr - prev) / prev
            }
        })
        .collect();

    if returns.is_empty() {
        return Decimal::ZERO;
    }

    let mean = returns.iter().copied().sum::<Decimal>() / Decimal::from(returns.len() as i64);

    let variance = returns
        .iter()
        .map(|r| {
            let diff = *r - mean;
            diff * diff
        })
        .sum::<Decimal>()
        / Decimal::from(returns.len() as i64);

    let std_dev = variance.sqrt().unwrap_or(Decimal::ZERO);
    if std_dev.is_zero() {
        return Decimal::ZERO;
    }

    // Intervalo mediano entre amostras → número de períodos por ano.
    let mut intervals: Vec<i64> = equity_series
        .windows(2)
        .map(|w| (w[1].0 - w[0].0).num_seconds())
        .filter(|s| *s > 0)
        .collect();
    intervals.sort_unstable();
    let median_secs = intervals
        .get(intervals.len() / 2)
        .copied()
        .unwrap_or(86_400);

    let secs_per_year = 365.25 * 86_400.0;
    let periods_per_year = secs_per_year / median_secs as f64;
    let annualizer =
        Decimal::from_f64_retain(periods_per_year.sqrt()).unwrap_or_else(|| Decimal::from(15));

    mean / std_dev * annualizer
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use trader_domain::{Direction, ExitReason};

    fn trade(net_pnl: i64, result_in_r: &str) -> Trade {
        let ts = Utc.with_ymd_and_hms(2026, 8, 3, 15, 0, 0).unwrap();
        Trade {
            id: None,
            symbol: "SPY".to_string(),
            signal_id: 1,
            position_id: None,
            direction: Direction::Long,
            entry_price: Decimal::from(100),
            exit_price: Decimal::from(101),
            quantity: Decimal::from(10),
            entry_time: ts,
            exit_time: ts,
            stop_price: Decimal::from(99),
            target_price: Some(Decimal::from(102)),
            gross_pnl: Decimal::from(net_pnl),
            commissions: Decimal::ZERO,
            fees: Decimal::ZERO,
            net_pnl: Decimal::from(net_pnl),
            risk_amount: Decimal::from(10),
            result_in_r: result_in_r.parse().unwrap(),
            exit_reason: ExitReason::Target,
            strategy_id: "test".to_string(),
            strategy_version: "1.0.0".to_string(),
            config_hash: "hash".to_string(),
            journal: serde_json::Value::Object(Default::default()),
            correlation_id: "corr".to_string(),
        }
    }

    fn com_saida(net_pnl: i64, result_in_r: &str, reason: ExitReason) -> Trade {
        Trade {
            exit_reason: reason,
            ..trade(net_pnl, result_in_r)
        }
    }

    /// ADR-018: sem a quebra por motivo de saída não dá para responder quanto
    /// do resultado vinha de posição encerrada no sino em vez de stop/alvo —
    /// a pergunta que separa o edge da estratégia do "ganhar dormindo".
    #[test]
    fn quebra_por_motivo_de_saida() {
        let trades = vec![
            com_saida(300, "3", ExitReason::Target),
            com_saida(-100, "-1", ExitReason::Stop),
            com_saida(200, "2", ExitReason::EndOfDay),
            com_saida(-50, "-0.5", ExitReason::EndOfDay),
        ];
        let m = BacktestMetrics::from_trades(&trades, Decimal::from(100_000));

        let eod = m
            .by_exit_reason
            .get("end_of_day")
            .expect("linha end_of_day");
        assert_eq!(eod.trades, 2);
        assert_eq!(eod.wins, 1);
        assert_eq!(eod.net_pnl, Decimal::from(150));
        assert_eq!(eod.profit_factor, Some(Decimal::from(4))); // 200 / 50
        assert_eq!(eod.avg_r, Decimal::new(75, 2)); // (2 − 0,5) / 2

        // Sem perdas no grupo, o PF é infinito e não zero.
        let alvo = m.by_exit_reason.get("target").expect("linha target");
        assert_eq!(alvo.trades, 1);
        assert_eq!(alvo.profit_factor, None);

        // Os grupos somam o total.
        assert_eq!(
            m.by_exit_reason.values().map(|g| g.trades).sum::<usize>(),
            m.total_trades
        );
        assert_eq!(
            m.by_exit_reason
                .values()
                .map(|g| g.net_pnl)
                .sum::<Decimal>(),
            m.net_pnl
        );
    }

    /// Trades gravados antes do ADR-018 marcavam o flatten como `manual` com
    /// a assinatura no journal — têm de cair na linha `end_of_day`, senão o
    /// `analyze` compara live e backtest em categorias diferentes.
    #[test]
    fn flatten_anterior_ao_adr_entra_como_end_of_day() {
        let mut antigo = com_saida(120, "1.2", ExitReason::Manual);
        antigo.journal = serde_json::json!({ "forced_exit": "session_flatten" });
        let m = BacktestMetrics::from_trades(&[antigo], Decimal::from(100_000));

        assert!(m.by_exit_reason.contains_key("end_of_day"));
        assert!(!m.by_exit_reason.contains_key("manual"));
    }

    fn em(net_pnl: i64, r: &str, dir: Direction, entrada: chrono::DateTime<Utc>) -> Trade {
        Trade {
            direction: dir,
            entry_time: entrada,
            exit_time: entrada + chrono::Duration::hours(1),
            ..trade(net_pnl, r)
        }
    }

    /// ADR-019 §4: PF em R e PF em $ divergem quando o tamanho da posicao
    /// covaria com o resultado -- que e exatamente o que o cap de notional
    /// produz neste projeto (balance OOS: PF$ 2,09 contra PF_R 1,41).
    #[test]
    fn pf_em_r_diverge_do_pf_em_dolares_quando_o_tamanho_covaria() {
        // Ganhador grande com R modesto; perdedor pequeno com R inteiro.
        let mut ganhador = trade(1000, "1");
        ganhador.risk_amount = Decimal::from(1000);
        let mut perdedor = trade(-100, "-1");
        perdedor.risk_amount = Decimal::from(100);

        let m = BacktestMetrics::from_trades(&[ganhador, perdedor], Decimal::from(100_000));
        assert_eq!(m.profit_factor, Some(Decimal::from(10))); // 1000 / 100
        assert_eq!(m.profit_factor_r, Some(Decimal::ONE)); // 1 / 1
        assert!(
            m.profit_factor.unwrap() > m.profit_factor_r.unwrap(),
            "o PF em dolares tem de ficar acima do PF em R neste desenho"
        );
    }

    /// A hora do bucket e a de NOVA YORK. Em UTC, 14h de julho e 14h de
    /// novembro sao horas de pregao diferentes -- e o veto que o A2 corrigiu.
    #[test]
    fn bucket_de_hora_usa_horario_de_ny_nao_utc() {
        let verao = Utc.with_ymd_and_hms(2026, 7, 22, 14, 15, 0).unwrap(); // 10h15 ET
        let inverno = Utc.with_ymd_and_hms(2026, 11, 2, 15, 15, 0).unwrap(); // 10h15 ET
        let m = BacktestMetrics::from_trades(
            &[
                em(100, "1", Direction::Long, verao),
                em(100, "1", Direction::Long, inverno),
            ],
            Decimal::from(100_000),
        );
        assert_eq!(
            m.by_entry_hour_et.keys().collect::<Vec<_>>(),
            vec!["10"],
            "as duas entradas sao 10h ET; em UTC cairiam em buckets diferentes"
        );
        assert_eq!(m.by_entry_hour_et["10"].trades, 2);
    }

    /// Concentracao e o risco mais subestimado do projeto: uma variante teve
    /// 64% do P&L num unico dia. Sem esta metrica o gate nao ve isso.
    #[test]
    fn concentracao_por_dia_e_por_mes() {
        let dia1 = Utc.with_ymd_and_hms(2026, 3, 10, 15, 0, 0).unwrap();
        let dia2 = Utc.with_ymd_and_hms(2026, 6, 20, 15, 0, 0).unwrap();
        let m = BacktestMetrics::from_trades(
            &[
                em(900, "3", Direction::Long, dia1),
                em(50, "1", Direction::Short, dia2),
                em(50, "1", Direction::Short, dia2),
            ],
            Decimal::from(100_000),
        );
        assert_eq!(m.trading_days, 2);
        assert_eq!(m.months_total, 2);
        assert_eq!(m.months_positive, 2);
        assert!(
            (m.top_day_share - 0.9).abs() < 1e-9,
            "melhor dia deveria ser 90% do net, veio {}",
            m.top_day_share
        );
        assert_eq!(m.by_direction["long"].trades, 1);
        assert_eq!(m.by_direction["short"].trades, 2);
    }

    /// t-stat separa "PF alto com 3 trades" de "PF alto com 80". Com menos de
    /// dois pontos a estatistica nao existe e o valor tem de ser 0, nao um
    /// numero grande que passaria por significancia.
    #[test]
    fn t_stat_exige_amostra() {
        assert_eq!(t_stat(&[]), 0.0);
        assert_eq!(t_stat(&[1.0]), 0.0);
        assert_eq!(
            t_stat(&[1.0, 1.0, 1.0]),
            0.0,
            "desvio zero nao e t infinito"
        );
        assert!(t_stat(&[1.0, 2.0, 3.0]) > 0.0);
    }

    #[test]
    fn correlacao_de_serie_constante_e_zero() {
        assert_eq!(correlacao(&[1.0, 1.0, 1.0], &[1.0, 2.0, 3.0]), 0.0);
        assert!((correlacao(&[1.0, 2.0, 3.0], &[2.0, 4.0, 6.0]) - 1.0).abs() < 1e-9);
        assert!((correlacao(&[1.0, 2.0, 3.0], &[3.0, 2.0, 1.0]) + 1.0).abs() < 1e-9);
    }

    /// Regressao: o jsonb `metrics` de `backtest_runs` guarda esta struct, e
    /// ha 700+ runs gravados ANTES do ADR-019 sem nenhum dos campos novos. Se
    /// algum deles perder o `#[serde(default)]`, o `analyze` passa a abortar
    /// com "metricas do run N invalidas" e o historico inteiro do gate B fica
    /// ilegivel -- em silencio, porque nada mais desserializa esta struct.
    ///
    /// O JSON abaixo e um recorte real do run 731 do banco dev (walk-forward
    /// da opening-reversal-v1 em IWM, gravado antes do ADR-019).
    #[test]
    fn metrics_de_run_antigo_ainda_desserializa() {
        let antigo = r#"{
            "net_pnl": "3772.85",
            "win_rate": "59.375",
            "best_trade": "946.25",
            "gross_loss": "5235.76",
            "worst_trade": "-559.08",
            "gross_profit": "9008.61",
            "max_drawdown": "2088.86",
            "sharpe_ratio": "0",
            "total_trades": 32,
            "losing_trades": 13,
            "profit_factor": "1.72",
            "winning_trades": 19,
            "avg_r_per_trade": "0.45",
            "max_drawdown_pct": "2.0",
            "avg_pnl_per_trade": "117.9",
            "max_consecutive_losses": 3
        }"#;

        let m: BacktestMetrics =
            serde_json::from_str(antigo).expect("run anterior ao ADR-019 tem de desserializar");

        assert_eq!(m.total_trades, 32);
        assert_eq!(m.profit_factor, Some(Decimal::new(172, 2)));
        // Os campos que nao existiam voltam zerados, nao quebram o parse.
        assert_eq!(m.profit_factor_r, None);
        assert_eq!(m.t_stat_avg_r, 0.0);
        assert_eq!(m.cost_total, Decimal::ZERO);
        assert!(m.by_exit_reason.is_empty());
        assert!(m.by_direction.is_empty());
        assert!(m.by_entry_hour_et.is_empty());
    }

    #[test]
    fn metrics_of_mixed_trades() {
        let trades = vec![
            trade(200, "2"),
            trade(-100, "-1"),
            trade(200, "2"),
            trade(-100, "-1"),
        ];
        let m = BacktestMetrics::from_trades(&trades, Decimal::from(100_000));

        assert_eq!(m.total_trades, 4);
        assert_eq!(m.winning_trades, 2);
        assert_eq!(m.win_rate, Decimal::from(50));
        assert_eq!(m.gross_profit, Decimal::from(400));
        assert_eq!(m.gross_loss, Decimal::from(200));
        assert_eq!(m.profit_factor, Some(Decimal::from(2)));
        assert_eq!(m.net_pnl, Decimal::from(200));
        assert_eq!(m.avg_pnl_per_trade, Decimal::from(50));
        assert_eq!(m.avg_r_per_trade, Decimal::new(5, 1)); // 0.5
        assert_eq!(m.max_consecutive_losses, 1);
        assert_eq!(m.best_trade, Decimal::from(200));
        assert_eq!(m.worst_trade, Decimal::from(-100));
    }

    #[test]
    fn drawdown_tracks_peak_to_valley() {
        // sobe 200, perde 100 → drawdown de 100 sobre pico de 100200.
        let trades = vec![trade(200, "2"), trade(-100, "-1")];
        let m = BacktestMetrics::from_trades(&trades, Decimal::from(100_000));

        assert_eq!(m.max_drawdown, Decimal::from(100));
        assert!(m.max_drawdown_pct > Decimal::ZERO);
    }

    #[test]
    fn empty_trades_yield_zeroed_metrics() {
        let m = BacktestMetrics::from_trades(&[], Decimal::from(100_000));
        assert_eq!(m.total_trades, 0);
        assert_eq!(m.profit_factor, None);
        assert_eq!(m.profit_factor_display(), "N/A");
    }

    #[test]
    fn all_winners_profit_factor_is_infinite() {
        let trades = vec![trade(200, "2"), trade(100, "1")];
        let m = BacktestMetrics::from_trades(&trades, Decimal::from(100_000));
        assert_eq!(m.profit_factor, None);
        assert_eq!(m.profit_factor_display(), "∞");
    }

    #[test]
    fn sharpe_annualizes_by_sample_interval() {
        // Série de equity com crescimento constante: variância zero → Sharpe 0.
        let base = Utc.with_ymd_and_hms(2026, 8, 3, 14, 30, 0).unwrap();
        let flat: Vec<(DateTime<Utc>, Decimal)> = (0..10)
            .map(|i| {
                (
                    base + chrono::Duration::minutes(i * 15),
                    Decimal::from(100_000),
                )
            })
            .collect();
        assert_eq!(calculate_sharpe(&flat), Decimal::ZERO);

        // Série com retornos variáveis a cada 15min: Sharpe deve ser bem maior
        // que a versão diária (√252 ≈ 15.9 vs √35040 ≈ 187).
        let mut equity = Decimal::from(100_000);
        let mut series = Vec::new();
        for i in 0..100 {
            let delta = if i % 2 == 0 { 100 } else { -50 };
            equity += Decimal::from(delta);
            series.push((base + chrono::Duration::minutes(i * 15), equity));
        }
        let sharpe_15m = calculate_sharpe(&series);
        assert!(sharpe_15m > Decimal::from(15));
    }
}
