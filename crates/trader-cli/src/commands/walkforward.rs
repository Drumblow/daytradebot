//! Comando `walkforward` — validação out-of-sample da estratégia.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use tracing::{info, warn};

use trader_backtest::{run_walk_forward, BacktestConfig};
use trader_domain::{CandleRepository, Strategy, TimeFrame};
use trader_infra::{
    db::create_pool,
    repositories::{BacktestRunRecord, SqlxBacktestRunRepository, SqlxCandleRepository},
};

use crate::config::CliConfig;

/// Argumentos do comando walkforward.
pub struct Args {
    pub symbol: String,
    pub strategy: String,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub timeframe: TimeFrame,
    /// Número de janelas out-of-sample.
    pub windows: usize,
    /// Desliga o flatten de fim de pregão (ADR-018).
    ///
    /// Só para reproduzir os runs 413–421 e medir o delta por par. Nenhum
    /// veredito de gate A sai de um run com esta flag.
    pub no_flatten: bool,
    /// Exporta o resultado completo (janelas + trades OOS) em JSON.
    pub output: Option<String>,
    /// Slippage por execução em pontos-base. `Decimal` de propósito: 2,5 bp
    /// precisa existir para a sensibilidade ao custo do ADR-016.
    pub slippage_bps: Option<Decimal>,
    /// Rótulo do run. **Obrigatório** quando houver override — sem ele a
    /// ablação vira baseline do gate B pelo `created_at` (ADR-019 §3).
    pub label: Option<String>,
    /// Bloco final travado (data ET). Nunca entra em seleção; roda uma vez
    /// por família de hipótese e é reportado em separado.
    pub holdout_from: Option<DateTime<Utc>>,
    /// TOML alternativo para a MESMA struct de parâmetros.
    pub strategy_config: Option<String>,
    /// Sobrescritas `chave=valor` em `[strategy.parameters]`.
    pub set: Vec<String>,
}

/// Bloco de saída do `--output`: o resultado do walk-forward mais o que
/// define o run.
///
/// Sem os metadados, dois JSONs de ablações diferentes são indistinguíveis —
/// que é o problema que o ADR-019 chama de "a primeira ablação vira baseline".
#[derive(serde::Serialize)]
struct WalkForwardOutput<'a> {
    symbol: &'a str,
    strategy_id: &'a str,
    strategy_version: &'a str,
    config_hash: &'a str,
    timeframe: String,
    windows: usize,
    slippage_bps: String,
    session_flatten: Option<String>,
    /// Base do `max_drawdown_pct` — o motor calcula o DD sobre o PICO, que
    /// comeca aqui. Sem este campo, quem le o JSON de fora (o
    /// `trader-research` do §5.3) precisa CHUTAR 100k, e o chute vira
    /// silenciosamente errado no dia em que o `capital_fraction` do ADR-020
    /// fracionar o capital do backtest.
    initial_capital: Decimal,
    /// Datas ET de TODOS os pregoes cobertos pela amostra OOS — inclusive os
    /// que nao tiveram trade.
    ///
    /// Sem isto, quem le o JSON so enxerga os dias COM trade (17 a 35 num
    /// periodo de ~270 pregoes) e o bootstrap em blocos do ADR-019 §8, que e
    /// pre-registrado sobre "o P&L diario de todos os pregoes, zeros
    /// incluidos", nao tem como ser executado como foi pre-registrado. A
    /// diferenca nao e cosmetica: reamostrar 19 pontos ou 272 muda o limite
    /// inferior do IC e chega a virar o veredito do criterio proposto.
    oos_sessions: Vec<chrono::NaiveDate>,
    label: &'a str,
    experimental: bool,
    overrides: Vec<(String, String)>,
    strategy_source: &'a str,
    holdout_from: Option<DateTime<Utc>>,
    selection: &'a trader_backtest::WalkForwardResult,
    holdout: Option<&'a trader_backtest::BacktestMetrics>,
    holdout_trades: Vec<trader_domain::Trade>,
}

/// Executa análise walk-forward (anchored) sobre dados reais do banco.
///
/// Sem dados reais, FALHA — walk-forward sobre dados sintéticos não é
/// evidência de nada.
pub async fn run(config: &CliConfig, args: Args) -> Result<()> {
    info!(
        symbol = %args.symbol,
        strategy = %args.strategy,
        windows = args.windows,
        "iniciando walk-forward"
    );

    println!("🔬 Iniciando walk-forward");
    println!("   Ativo:     {}", args.symbol);
    println!("   Estratégia: {}", args.strategy);
    println!("   Timeframe: {}", args.timeframe);
    println!("   Janelas:   {}", args.windows);

    // ADR-019 §1/§2: a config pode vir do TOML canônico, de um alternativo
    // (`--strategy-config`) ou do canônico com `--set`. As travas contra
    // "ablação barata vira baseline em silêncio" ficam todas aqui.
    let resolvido = crate::strategy_source::resolve(
        &args.strategy,
        args.strategy_config.as_deref(),
        &args.set,
    )?;
    let strategy = crate::dispatch::load_strategy(&args.strategy, &resolvido.toml)?;

    if resolvido.is_experimental {
        // Sem rótulo, o `analyze` escolhe o baseline do gate B por
        // `created_at` e a primeira ablação toma o lugar da produção.
        if args.label.is_none() {
            anyhow::bail!(
                "--label é obrigatório com --set/--strategy-config: sem ele este run \
                 entra no histórico indistinguível do run de produção e pode virar \
                 baseline do gate B por ordem de chegada (ADR-019 §3)."
            );
        }
        // O `config_hash` sai da struct JÁ desserializada: chave inexistente é
        // descartada no parse e o hash não muda. Se não mudou, o `--set` não
        // fez nada e o rótulo mentiria.
        if !args.set.is_empty() {
            let canonico = crate::strategy_source::canonical_config_hash(&args.strategy)?;
            if strategy.config_hash() == canonico {
                anyhow::bail!(
                    "o --set não alterou a configuração (config_hash continua {canonico}). \
                     Ou a chave não existe, ou o valor é igual ao do arquivo — nos dois \
                     casos o run seria rotulado como ablação sem ser uma."
                );
            }
        }
        if args.holdout_from.is_some() {
            anyhow::bail!(
                "--holdout-from não pode ser usado com --set/--strategy-config: o holdout \
                 roda UMA vez por família de hipótese e não participa de seleção \
                 (ADR-019 §1)."
            );
        }
        println!("   ⚗️  Experimental: {}", resolvido.source);
        for (k, v) in &resolvido.overrides {
            println!("      --set {k}={v}");
        }
    }

    let database_url = config
        .app_config
        .database
        .url()
        .map_err(|e| anyhow::anyhow!("DATABASE_URL não configurada: {e}"))?;
    let pool = create_pool(&database_url)
        .await
        .map_err(|e| anyhow::anyhow!("falha ao conectar no banco: {e}"))?;

    let to = args.to.unwrap_or_else(Utc::now);
    let from = args
        .from
        .unwrap_or_else(|| to - chrono::Duration::days(180));

    let repo = SqlxCandleRepository::new(pool.clone());
    let candles = repo
        .get_range(&args.symbol, args.timeframe, from, to)
        .await
        .map_err(|e| anyhow::anyhow!("falha ao buscar candles: {e}"))?;

    if candles.is_empty() {
        anyhow::bail!(
            "nenhum candle no banco para {} no timeframe {}. \
             Rode 'trader-cli ingest' primeiro — walk-forward exige dados reais.",
            args.symbol,
            args.timeframe
        );
    }
    println!(
        "   Candles:   {} ({} → {})\n",
        candles.len(),
        from.date_naive(),
        to.date_naive()
    );

    // Holdout travado (ADR-019 §1, plano §3.4). O walk-forward do repo NÃO é
    // OOS em relação ao desenho das regras — as estratégias são funções puras
    // e nada é re-ajustado, então o "OOS" é a mesma rodada determinística
    // depois do primeiro bloco. O holdout é a única fatia que nenhuma seleção
    // toca, e por isso tem de sair da série ANTES de `split_windows`: se
    // ficasse, entraria nos blocos de treino e deixaria de ser holdout.
    let (selecao, holdout_candles) = match args.holdout_from {
        Some(corte) => {
            let idx = candles.partition_point(|c| c.timestamp < corte);
            if idx == 0 {
                anyhow::bail!(
                    "--holdout-from {} é anterior ao primeiro candle ({}): não sobra \
                     nada para a seleção.",
                    corte.date_naive(),
                    candles[0].timestamp.date_naive()
                );
            }
            if idx == candles.len() {
                anyhow::bail!(
                    "--holdout-from {} é posterior ao último candle ({}): o holdout \
                     ficaria vazio.",
                    corte.date_naive(),
                    candles[candles.len() - 1].timestamp.date_naive()
                );
            }
            println!(
                "   🔒 Holdout:  {} candles a partir de {} — travados, fora da seleção\n",
                candles.len() - idx,
                corte.date_naive()
            );
            (&candles[..idx], Some(&candles[idx..]))
        }
        None => (&candles[..], None),
    };

    let backtest_config = BacktestConfig {
        symbol: args.symbol.clone(),
        entry_validity_candles: strategy.entry_validity_candles() as u32,
        time_exit: strategy.time_exit(),
        session_flatten_et: super::session_flatten_et(&config.app_config.session, args.no_flatten),
        // Até o ADR-019 o walk-forward herdava 2 bp do `default()` sem ninguém
        // poder mudar: toda sensibilidade ao custo (ADR-016) tinha de ser
        // rodada no `backtest`, com outra régua.
        slippage_pct: args
            .slippage_bps
            .map(|bps| bps / Decimal::from(10_000))
            .unwrap_or_else(|| BacktestConfig::default().slippage_pct),
        ..BacktestConfig::default()
    };
    if args.no_flatten {
        println!(
            "   ⚠️  Flatten:   DESLIGADO (--no-flatten) — régua anterior ao ADR-018, \
             só para comparação. Não é veredito de gate A.\n"
        );
    }
    let risk_config =
        crate::risk_config::build_risk_config(&config.app_config.risk, &strategy.risk_params())?;

    let result = run_walk_forward(
        &strategy,
        selecao,
        args.windows,
        &backtest_config,
        risk_config,
    )
    .await?;

    // O calendario de pregoes que a amostra OOS cobriu, do jeito que o motor
    // viu: a uniao das janelas de teste do `split_windows` e a data ET de
    // cada candle nela. Nao da para reconstruir isso de fora com dias uteis —
    // feriado de NYSE e dia de meio pregao entrariam como pregao.
    let oos_sessions: Vec<chrono::NaiveDate> = {
        let mut datas: Vec<chrono::NaiveDate> =
            match trader_backtest::split_windows(selecao.len(), args.windows) {
                Some(splits) => {
                    let inicio = splits.first().map(|(_, teste)| teste.start).unwrap_or(0);
                    let fim = splits.last().map(|(_, teste)| teste.end).unwrap_or(0);
                    selecao[inicio..fim]
                        .iter()
                        .map(|c| trader_core::session::et_date(c.timestamp))
                        .collect()
                }
                None => Vec::new(),
            };
        datas.sort_unstable();
        datas.dedup();
        datas
    };

    // O holdout roda como UM backtest sobre a série inteira (o warm-up dos
    // indicadores precisa dos candles anteriores) e conta só os trades que
    // entram depois do corte. Nunca participou de seleção nenhuma.
    let (holdout_metrics, holdout_trades) = match (args.holdout_from, holdout_candles) {
        (Some(corte), Some(_)) => {
            let mut engine =
                trader_backtest::BacktestEngine::new(backtest_config.clone(), risk_config);
            let run = engine.run(&strategy, &candles).await?;
            let trades: Vec<trader_domain::Trade> = run
                .closed_trades
                .into_iter()
                .filter(|t| t.entry_time >= corte)
                .collect();
            let m = trader_backtest::BacktestMetrics::from_trades(
                &trades,
                backtest_config.initial_capital,
            );
            (Some(m), trades)
        }
        _ => (None, Vec::new()),
    };

    // Relatório por janela: degradação IS → OOS indica sobreajuste/regime.
    println!("{:-<100}", "");
    println!(
        "{:^7} {:^22} {:^22} {:^8} {:^8} {:^8} {:^8} {:^8}",
        "janela", "período teste", "", "trades", "win%", "PF", "avgR", "netP&L"
    );
    for w in &result.windows {
        println!(
            "{:^7} {} → {}  IS {:^4} OOS {:^4} {:^8} {:^8} {:^8.2} {:^10.2}",
            w.window,
            w.test_start.date_naive(),
            w.test_end.date_naive(),
            w.in_sample.total_trades,
            w.out_of_sample.total_trades,
            format!("{:.1}", w.out_of_sample.win_rate),
            w.out_of_sample.profit_factor_display(),
            w.out_of_sample.avg_r_per_trade,
            w.out_of_sample.net_pnl,
        );
    }
    println!("{:-<100}", "");

    let m = &result.oos_metrics;
    println!("📊 Out-of-sample agregado (a amostra que conta):");
    println!("   Trades:        {}", m.total_trades);
    println!("   Win rate:      {}%", m.win_rate);
    println!("   Profit factor: {}", m.profit_factor_display());
    println!("   Avg R/trade:   {:.3}", m.avg_r_per_trade);
    println!(
        "   Max drawdown:  {} ({}%)",
        m.max_drawdown, m.max_drawdown_pct
    );
    println!("   Net P&L:       {}", m.net_pnl);
    println!();
    println!(
        "   Critérios de aceitação (docs/strategies/{}.md):",
        strategy.id().id
    );
    print_acceptance(m.total_trades, m);

    // O holdout é reportado SEPARADO e nunca somado à seleção: misturá-los
    // desfaria a única fatia que nenhuma escolha tocou.
    if let Some(h) = &holdout_metrics {
        println!("\n🔒 Holdout travado (rodado UMA vez, fora de qualquer seleção):");
        println!("   Trades:        {}", h.total_trades);
        println!("   Win rate:      {:.1}%", h.win_rate);
        println!("   Profit factor: {}", h.profit_factor_display());
        println!("   Avg R/trade:   {:.3}", h.avg_r_per_trade);
        println!("   Net P&L:       {:.2}", h.net_pnl);
        println!();
        print_acceptance(h.total_trades, h);
    }

    let slippage_bps = (backtest_config.slippage_pct * Decimal::from(10_000)).normalize();
    let label = args
        .label
        .clone()
        .unwrap_or_else(|| format!("walkforward-oos-{}w", args.windows));

    if let Some(path) = &args.output {
        let strategy_id = strategy.id();
        let config_hash = strategy.config_hash();
        let saida = WalkForwardOutput {
            symbol: &args.symbol,
            strategy_id: &strategy_id.id,
            strategy_version: &strategy_id.version,
            config_hash: &config_hash,
            timeframe: format!("{:?}", args.timeframe),
            windows: args.windows,
            slippage_bps: slippage_bps.to_string(),
            session_flatten: backtest_config
                .session_flatten_et
                .map(|(h, mi)| format!("{h:02}:{mi:02}")),
            initial_capital: backtest_config.initial_capital,
            oos_sessions: oos_sessions.clone(),
            label: &label,
            experimental: resolvido.is_experimental,
            overrides: resolvido.overrides.clone(),
            strategy_source: &resolvido.source,
            holdout_from: args.holdout_from,
            selection: &result,
            holdout: holdout_metrics.as_ref(),
            holdout_trades,
        };
        let json = serde_json::to_string_pretty(&saida)
            .context("falha ao serializar o resultado do walk-forward")?;
        std::fs::write(path, json)
            .with_context(|| format!("falha ao escrever relatório em {path}"))?;
        println!("\n   Resultado exportado para {path}");
    }

    // Persiste o run agregado OOS para histórico.
    //
    // O jsonb `metrics` ganha o que define o run além dos números (ADR-019
    // §3): sem `slippage_bps`, `overrides` e `session_flatten` gravados, dois
    // runs com custo ou régua diferentes são indistinguíveis no banco e o
    // `analyze` pode pegar o errado como baseline do gate B.
    let mut metrics_json =
        serde_json::to_value(m).unwrap_or(serde_json::Value::Object(Default::default()));
    if let Some(obj) = metrics_json.as_object_mut() {
        obj.insert("slippage_bps".into(), slippage_bps.to_string().into());
        obj.insert(
            "session_flatten".into(),
            match backtest_config.session_flatten_et {
                Some((h, mi)) => format!("{h:02}:{mi:02}").into(),
                None => serde_json::Value::Null,
            },
        );
        obj.insert("experimental".into(), resolvido.is_experimental.into());
        obj.insert(
            "overrides".into(),
            serde_json::Value::Object(
                resolvido
                    .overrides
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                    .collect(),
            ),
        );
        obj.insert("windows".into(), args.windows.into());
        obj.insert(
            "holdout_from".into(),
            match args.holdout_from {
                Some(d) => d.to_rfc3339().into(),
                None => serde_json::Value::Null,
            },
        );
        if let Some(h) = &holdout_metrics {
            obj.insert(
                "holdout_metrics".into(),
                serde_json::to_value(h).unwrap_or(serde_json::Value::Null),
            );
        }
    }

    let record = BacktestRunRecord {
        symbol: args.symbol.clone(),
        strategy_id: strategy.id().id,
        strategy_version: strategy.id().version,
        config_hash: strategy.config_hash(),
        timeframe: format!("{:?}", args.timeframe),
        period_start: candles.first().map(|c| c.timestamp).unwrap_or(from),
        period_end: candles.last().map(|c| c.timestamp).unwrap_or(to),
        initial_capital: backtest_config.initial_capital,
        final_equity: backtest_config.initial_capital + m.net_pnl,
        metrics: metrics_json,
        label: Some(label),
    };
    let run_repo = SqlxBacktestRunRepository::new(pool);
    match run_repo.save(&record).await {
        Ok(id) => println!("\n   Run OOS persistido no banco (id={})", id),
        Err(e) => warn!(error = %e, "falha ao persistir run de walk-forward"),
    }

    Ok(())
}

/// Imprime o veredito contra os critérios de aceitação do backtest.
fn print_acceptance(total_trades: usize, m: &trader_backtest::BacktestMetrics) {
    let check =
        |ok: bool, label: String| println!("   [{}] {}", if ok { "OK" } else { "--" }, label);

    check(
        total_trades >= 50,
        format!("≥ 50 trades (atual: {total_trades})"),
    );
    check(
        m.win_rate >= Decimal::from(40),
        format!("win rate ≥ 40% (atual: {:.1}%)", m.win_rate),
    );
    check(
        // Sem perdas no período (None) o PF é infinito: critério atendido.
        m.profit_factor
            .map(|pf| pf >= Decimal::new(13, 1))
            .unwrap_or(m.total_trades > 0),
        format!("profit factor ≥ 1.3 (atual: {})", m.profit_factor_display()),
    );
    check(
        m.max_drawdown_pct <= Decimal::from(10),
        format!("drawdown ≤ 10% (atual: {:.2}%)", m.max_drawdown_pct),
    );
    check(
        m.avg_r_per_trade > Decimal::new(15, 2),
        format!("avg R > 0.15 (atual: {:.3})", m.avg_r_per_trade),
    );
    check(
        m.net_pnl > Decimal::ZERO,
        format!("expectativa positiva (net P&L: {:.2})", m.net_pnl),
    );

    // Critérios propostos pelo ADR-019 §7. Impressos como PROPOSTA enquanto o
    // dono não os adotar formalmente (decisão 2 do §10 do plano): o veredito
    // do ADR-010 continua sendo o de cima.
    println!("   --- proposta ADR-019 §7 (ainda não é o gate vigente) ---");
    check(
        m.profit_factor_r
            .map(|pf| pf >= Decimal::new(12, 1))
            .unwrap_or(m.total_trades > 0),
        format!(
            "PF em R ≥ 1.2 (atual: {}) — o PF em $ infla com o sizing",
            m.profit_factor_r_display()
        ),
    );
    check(
        m.top2_month_share <= 0.60,
        format!(
            "2 melhores meses ≤ 60% do net (atual: {:.0}%, em {} meses, {} positivos)",
            m.top2_month_share * 100.0,
            m.months_total,
            m.months_positive
        ),
    );

    // Relatório obrigatório, não critério (plano §3.5): com < 100 trades
    // nenhuma estratégia deste projeto alcança DSR 0,95, então estes números
    // informam a leitura em vez de aprovar ou reprovar sozinhos.
    println!(
        "   [ i] t-stat do avg R: {:.2} · corr(risco, R): {:.2} · custo: {:.2}",
        m.t_stat_avg_r, m.corr_risk_result, m.cost_total
    );
    println!(
        "   [ i] concentração: melhor dia {:.0}% · top-5 dias {:.0}% · {} pregões com trade",
        m.top_day_share * 100.0,
        m.top5_day_share * 100.0,
        m.trading_days
    );
    if !m.by_direction.is_empty() {
        let lado = |k: &str| {
            m.by_direction
                .get(k)
                .map(|g| {
                    format!(
                        "{} t / PF {}",
                        g.trades,
                        g.profit_factor
                            .map(|pf| format!("{pf:.2}"))
                            .unwrap_or_else(|| "∞".into())
                    )
                })
                .unwrap_or_else(|| "-".into())
        };
        println!("   [ i] long: {} · short: {}", lado("long"), lado("short"));
    }
    // Sob a hipótese nula, com n≈25 por combinação, P(PF ≥ 1,3 | sem edge) =
    // 0,20–0,28 (plano §3.3). O número acompanha todo veredito, por decisão.
    if m.total_trades < 50 {
        println!(
            "   [ !] amostra de {} trades: sob a hipótese nula, P(PF ≥ 1,3 | sem edge)              fica em 0,20–0,28 com n≈25. Nenhum veredito aqui é conclusivo.",
            m.total_trades
        );
    }
}
