//! Round-trip Trade → banco → Trade.
//!
//! Existe por causa do ADR-018: o CHECK de `trades.exit_reason`, o `match` de
//! escrita e o de leitura são três tabelas que precisam concordar, e **nada
//! mais no projeto as compara**. A escrita quebra a compilação se divergir; a
//! leitura, não. Antes do ADR-018 ela tinha um `_ => Target` que transformava
//! qualquer texto desconhecido em "alvo" nas métricas do gate, em silêncio.

use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::PgPool;

use trader_domain::{Direction, ExitReason, Trade};
use trader_infra::repositories::SqlxTradeRepository;

/// `trades.signal_id` tem FK para `signals`; o teste precisa de um sinal real
/// antes do trade. Query dinâmica (não a macro) de propósito: é fixture de
/// teste e não deve entrar no cache `.sqlx` do build offline.
async fn signal_fixture(pool: &PgPool) -> i64 {
    let asset_id: i32 = sqlx::query_scalar(
        "INSERT INTO assets (symbol, name, asset_type, exchange, currency, tick_size)
         VALUES ('IJS', 'iShares S&P SmallCap 600 Value', 'etf', 'ARCA', 'USD', 0.01)
         ON CONFLICT (symbol) DO UPDATE SET symbol = EXCLUDED.symbol
         RETURNING id",
    )
    .fetch_one(pool)
    .await
    .expect("criar ativo");

    sqlx::query_scalar(
        "INSERT INTO signals (
            asset_id, strategy_id, strategy_version, config_hash, timeframe,
            timestamp, direction, status, market_snapshot, correlation_id
         )
         VALUES ($1, 'balance-area-breakout-v1', '1.0.0', 'deadbeefdeadbeef', '15m',
                 NOW(), 'long', 'accepted', '{}'::jsonb, gen_random_uuid())
         RETURNING id",
    )
    .bind(asset_id)
    .fetch_one(pool)
    .await
    .expect("criar sinal")
}

fn trade(signal_id: i64, exit_reason: ExitReason, journal: serde_json::Value) -> Trade {
    let now = Utc::now();
    Trade {
        id: None,
        symbol: "IJS".to_string(),
        signal_id,
        position_id: None,
        direction: Direction::Long,
        entry_price: Decimal::new(10050, 2),
        exit_price: Decimal::new(10125, 2),
        quantity: Decimal::from(800),
        entry_time: now - chrono::Duration::hours(2),
        exit_time: now,
        stop_price: Decimal::new(10025, 2),
        target_price: Some(Decimal::new(10100, 2)),
        gross_pnl: Decimal::from(600),
        commissions: Decimal::new(800, 2),
        fees: Decimal::ZERO,
        net_pnl: Decimal::new(59200, 2),
        risk_amount: Decimal::from(200),
        result_in_r: Decimal::new(296, 2),
        exit_reason,
        strategy_id: "balance-area-breakout-v1".to_string(),
        strategy_version: "1.0.0".to_string(),
        config_hash: "deadbeefdeadbeef".to_string(),
        journal,
        correlation_id: uuid::Uuid::new_v4().to_string(),
    }
}

/// Toda variante de `ExitReason` sobrevive à ida e à volta.
///
/// `end_of_day` é a que a migração 0004 acrescentou: se o CHECK não tiver sido
/// ampliado, o INSERT falha aqui (SQLSTATE 23514) em vez de falhar só em
/// produção; se o braço de leitura sumir, a asserção pega.
#[sqlx::test(migrations = "src/db/migrations")]
async fn todo_exit_reason_faz_round_trip(pool: PgPool) {
    let signal_id = signal_fixture(&pool).await;
    let repo = SqlxTradeRepository::new(pool);

    for reason in [
        ExitReason::Target,
        ExitReason::Stop,
        ExitReason::Time,
        ExitReason::Manual,
        ExitReason::RiskManager,
        ExitReason::EndOfDay,
    ] {
        let id = repo
            .save(&trade(
                signal_id,
                reason,
                serde_json::json!({ "source": "teste" }),
            ))
            .await
            .unwrap_or_else(|e| panic!("salvar trade com {reason:?}: {e}"));

        let lido = repo
            .get_by_id(id)
            .await
            .expect("buscar trade")
            .expect("trade existe");

        assert_eq!(
            lido.exit_reason, reason,
            "{reason:?} não sobreviveu ao round-trip (veio {:?})",
            lido.exit_reason
        );
        assert_eq!(lido.symbol, "IJS");
        assert_eq!(lido.net_pnl, Decimal::new(59200, 2));
    }
}

/// O flatten gravado antes do ADR-018 (`manual` + marca no journal) continua
/// sendo lido como `Manual` — o banco guarda o que foi gravado — mas
/// `effective_exit_reason` o reconhece como fim de pregão. É o que evita que o
/// `analyze` do gate B misture categorias sem reescrever linha nenhuma.
#[sqlx::test(migrations = "src/db/migrations")]
async fn flatten_legado_e_reconhecido_sem_reescrever_o_banco(pool: PgPool) {
    let signal_id = signal_fixture(&pool).await;
    let repo = SqlxTradeRepository::new(pool);

    let id = repo
        .save(&trade(
            signal_id,
            ExitReason::Manual,
            serde_json::json!({ "source": "live_fills", "forced_exit": "session_flatten" }),
        ))
        .await
        .expect("salvar trade legado");

    let lido = repo.get_by_id(id).await.unwrap().unwrap();

    assert_eq!(
        lido.exit_reason,
        ExitReason::Manual,
        "o banco não é reescrito"
    );
    assert_eq!(
        lido.effective_exit_reason(),
        ExitReason::EndOfDay,
        "a métrica tem de enxergar o flatten legado"
    );
}
