//! Comando `backtest`.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use tracing::{info, warn};

use trader_backtest::{BacktestConfig, BacktestEngine, BacktestReport};
use trader_domain::{CandleRepository, Strategy, TimeFrame};
use trader_infra::{
    db::create_pool,
    repositories::{BacktestRunRecord, SqlxBacktestRunRepository, SqlxCandleRepository},
};

use crate::config::CliConfig;

/// Argumentos do comando backtest.
pub struct Args {
    pub symbol: String,
    pub strategy: String,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub timeframe: TimeFrame,
    /// Permite rodar sobre série sintética quando não há dados no banco.
    /// Sem esta flag, backtest sem dados reais FALHA — um backtest sobre
    /// dados fabricados não é evidência de nada.
    pub allow_synthetic: bool,
    /// Caminho opcional para exportar o relatório em JSON.
    pub output: Option<String>,
    /// Slippage por execução, em pontos-base do preço (1 bp = 0,01%).
    ///
    /// Existe para CALIBRAR, não para embelezar: o custo de execução destas
    /// estratégias é da mesma ordem de grandeza do risco por trade (stops de
    /// ~0,13% do preço), então o resultado é muito sensível a ele. Varie e
    /// veja onde o PF cruza 1 antes de acreditar em qualquer backtest.
    pub slippage_bps: Option<u32>,
    /// Desliga o flatten de fim de pregão (ADR-018).
    ///
    /// Só para reproduzir runs anteriores ao ADR e medir o delta por par. Um
    /// backtest com `--no-flatten` carrega posição pela noite — coisa que o
    /// live, cujas pernas de bracket vão com TIF Day, nunca faz. Não use o
    /// número dele para julgar estratégia.
    pub no_flatten: bool,
    /// Restaura o custo anterior ao §5.6: comissão de US$ 0,35 fixos por
    /// perna e alvo limite que enche sem pagar nada.
    ///
    /// Só para reproduzir runs de até 08/09/2026. A IBKR cobra por AÇÃO
    /// (US$ 0,005, mín. US$ 1,00): nos 214 trades OOS medidos (234 a 1.311
    /// ações, mediana 619) isso é US$ 2,34 a 13,11 por trade, não US$ 0,70.
    pub legacy_cost: bool,
    /// Desconto no fill do alvo, em pontos-base. Sobrepõe o padrão (2 bp) e o
    /// modo legado (0). É o parâmetro escolhido a mão do modelo de custo.
    pub limit_haircut_bps: Option<Decimal>,
    /// Rótulo do run no banco.
    ///
    /// Sem isto o `backtest` gravava `label = NULL` SEMPRE — 586 dos 745 runs
    /// do banco dev não têm rótulo, e a maior parte veio daqui. O `--label` do
    /// `walkforward` existe desde o ADR-019; este comando ficou de fora.
    pub label: Option<String>,
    /// Modo de dimensionamento (ADR-020 §6). Torna o run experimental.
    pub sizing: super::SizingOverrides,
}

/// Executa um backtest da estratégia solicitada.
///
/// Usa o mesmo `RiskConfig` do live/paper (paridade de validação) e persiste
/// o run no banco (`backtest_runs`) para comparação futura.
pub async fn run(config: &CliConfig, args: Args) -> Result<()> {
    info!(
        symbol = %args.symbol,
        strategy = %args.strategy,
        "iniciando backtest"
    );

    println!("📈 Iniciando backtest");
    println!("   Ativo:     {}", args.symbol);
    println!("   Estratégia: {}", args.strategy);
    println!("   Timeframe: {}", args.timeframe);

    // Carrega configuração da estratégia.
    let strategy_path = format!("config/strategies/{}.toml", args.strategy);
    let strategy_toml = std::fs::read_to_string(&strategy_path)
        .with_context(|| format!("falha ao ler config da estratégia em {}", strategy_path))?;

    let strategy = crate::dispatch::load_strategy(&args.strategy, &strategy_toml)?;

    let pool = match config.app_config.database.url() {
        Ok(url) => match create_pool(&url).await {
            Ok(pool) => Some(pool),
            Err(e) => {
                warn!(error = %e, "falha ao conectar no banco");
                None
            }
        },
        Err(e) => {
            warn!(error = %e, "DATABASE_URL não configurada");
            None
        }
    };

    let candles = match &pool {
        Some(pool) => {
            let loaded = load_candles(pool, &args).await?;
            if loaded.is_empty() {
                if !args.allow_synthetic {
                    anyhow::bail!(
                        "nenhum candle no banco para {} no timeframe {}. \
                         Rode 'trader-cli ingest' primeiro, ou use --allow-synthetic \
                         para um smoke test com dados fabricados.",
                        args.symbol,
                        args.timeframe
                    );
                }
                warn!("nenhum candle no banco; usando série sintética (--allow-synthetic)");
                println!("   Fonte:      sintética (--allow-synthetic)");
                generate_synthetic_series(&args.symbol)
            } else {
                println!("   Fonte:      banco de dados ({} candles)", loaded.len());
                loaded
            }
        }
        None => {
            if !args.allow_synthetic {
                anyhow::bail!(
                    "sem banco de dados disponível. Configure DATABASE_URL e rode \
                     'trader-cli ingest', ou use --allow-synthetic para um smoke test."
                );
            }
            warn!("sem banco; usando série sintética (--allow-synthetic)");
            println!("   Fonte:      sintética (--allow-synthetic)");
            generate_synthetic_series(&args.symbol)
        }
    };

    let backtest_config = BacktestConfig {
        symbol: args.symbol.clone(),
        initial_capital: Decimal::from(100_000),
        commission: super::commission_model(args.legacy_cost),
        limit_fill_haircut_pct: super::limit_fill_haircut(args.legacy_cost, args.limit_haircut_bps),
        slippage_pct: match args.slippage_bps {
            Some(bps) => Decimal::from(bps) / Decimal::from(10_000),
            // 2 bp — ver a justificativa da calibracao em
            // trader-backtest/src/engine.rs.
            None => Decimal::from(2) / Decimal::from(10_000),
        },
        entry_validity_candles: strategy.entry_validity_candles() as u32,
        time_exit: strategy.time_exit(),
        session_flatten_et: super::session_flatten_et(&config.app_config.session, args.no_flatten),
    };

    if args.no_flatten {
        println!(
            "   ⚠️  Flatten:   DESLIGADO (--no-flatten) — posições atravessam a noite, \
             o que o live não faz. Número só serve de comparação."
        );
    }
    if args.legacy_cost {
        println!("   Custo:      LEGADO (--legacy-cost): US$ 0,35 fixos por perna");
        println!("               e alvo limite que enche de graça. Reproduz runs");
        println!("               de até 08/09/2026; não é o custo da IBKR.");
    } else {
        println!("   Custo:      IBKR por ação (US$ 0,005, mín. US$ 1,00) + 2 bp no alvo");
    }

    // Guardados antes do move para o engine: identificam a régua do run no
    // `metrics` persistido (ADR-019 §3).
    let slippage_bps_run = (backtest_config.slippage_pct * Decimal::from(10_000)).normalize();
    let session_flatten_run = backtest_config.session_flatten_et;
    let commission_run = super::commission_label(args.legacy_cost);
    let haircut_bps_run =
        (backtest_config.limit_fill_haircut_pct * Decimal::from(10_000)).normalize();

    // Paridade com o live: mesmos limites de risco e horário da estratégia.
    let risk_settings = args.sizing.aplica(&config.app_config.risk)?;
    let mut risk_params = strategy.risk_params();
    if args.sizing.risk_pct.is_some() && risk_params.risk_per_trade_pct.is_some() {
        println!(
            "   ⚖️  --risk-pct sobrepõe o override da estratégia ({} → {})",
            risk_params.risk_per_trade_pct.unwrap_or_default(),
            args.sizing.risk_pct.unwrap_or_default()
        );
        risk_params.risk_per_trade_pct = None;
    }
    let risk_config = crate::risk_config::build_risk_config(&risk_settings, &risk_params)?;
    if let Some(aviso) = crate::risk_config::aviso_de_fracao(&risk_settings) {
        println!("   ⚠️  {aviso}");
    }
    if !args.sizing.vazio() {
        println!("   ⚖️  Sizing (ADR-020):");
        for (k, v) in args.sizing.descricao() {
            println!("      {k} = {v}");
        }
    }
    let mut engine = BacktestEngine::new(backtest_config, risk_config);

    let run = engine.run(&strategy, &candles).await?;
    let report = BacktestReport::from_run(run);

    println!("{}", report);

    // Exporta o relatório em JSON, se solicitado.
    if let Some(path) = &args.output {
        let json = report.to_json()?;
        std::fs::write(path, json)
            .with_context(|| format!("falha ao escrever relatório em {}", path))?;
        println!("   Relatório exportado para {}", path);
    }

    // Persiste o run no banco (melhor esforço: backtest já foi executado).
    if let Some(pool) = &pool {
        // Mesmo enriquecimento do `walkforward` (ADR-019 §3). Sem ele, um run
        // de `backtest` fica indistinguível de outro rodado a custo ou régua
        // diferentes — e ele é elegível a baseline do gate B pelo `latest_for`
        // tanto quanto um de walk-forward.
        let mut metrics_json = serde_json::to_value(&report.metrics)
            .unwrap_or(serde_json::Value::Object(Default::default()));
        if let Some(obj) = metrics_json.as_object_mut() {
            obj.insert("slippage_bps".into(), slippage_bps_run.to_string().into());
            obj.insert(
                "session_flatten".into(),
                match session_flatten_run {
                    Some((h, m)) => format!("{h:02}:{m:02}").into(),
                    None => serde_json::Value::Null,
                },
            );
            // O `backtest` não tem `--set`, mas TEM modo de sizing (ADR-020
            // §6): um run de modo não pode virar baseline do gate B.
            obj.insert("experimental".into(), (!args.sizing.vazio()).into());
            obj.insert("sizing".into(), super::sizing_json(&risk_config));
            obj.insert("commission_model".into(), commission_run.into());
            obj.insert(
                "limit_fill_haircut_bps".into(),
                haircut_bps_run.to_string().into(),
            );
        }

        let record =
            BacktestRunRecord {
                symbol: args.symbol.clone(),
                strategy_id: strategy.id().id,
                strategy_version: strategy.id().version,
                config_hash: strategy.config_hash(),
                timeframe: format!("{:?}", args.timeframe),
                period_start: report.start_time,
                period_end: report.end_time,
                initial_capital: report.initial_capital,
                final_equity: report.final_equity,
                metrics: metrics_json,
                // Sem `--label`, grava a data em vez de NULL: um run anônimo é
                // impossível de atribuir depois, e era assim que 586 dos 745 runs
                // do banco ficaram (§5.6 do plano).
                label: Some(args.label.clone().unwrap_or_else(|| {
                    format!("backtest-{}", chrono::Utc::now().format("%Y-%m-%d"))
                })),
            };
        let repo = SqlxBacktestRunRepository::new(pool.clone());
        match repo.save(&record).await {
            Ok(id) => println!("   Run persistido no banco (id={})", id),
            Err(e) => warn!(error = %e, "falha ao persistir run de backtest"),
        }
    }

    Ok(())
}

async fn load_candles(pool: &sqlx::PgPool, args: &Args) -> Result<Vec<trader_domain::Candle>> {
    let repo = SqlxCandleRepository::new(pool.clone());

    let to = args.to.unwrap_or_else(Utc::now);
    let from = args
        .from
        .unwrap_or_else(|| to - chrono::Duration::days(180));

    repo.get_range(&args.symbol, args.timeframe, from, to)
        .await
        .map_err(|e| anyhow::anyhow!("falha ao buscar candles: {e}"))
}

fn generate_synthetic_series(symbol: &str) -> Vec<trader_domain::Candle> {
    let mut candles = crate::synthetic::generate_synthetic_uptrend(symbol);

    // Adiciona candles de continuação para que o alvo seja atingido.
    for _ in 0..20 {
        if let Some(next) = crate::synthetic::next_candle(symbol, &candles) {
            candles.push(next);
        }
    }

    candles
}
