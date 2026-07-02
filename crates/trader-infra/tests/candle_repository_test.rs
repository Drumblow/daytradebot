use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::PgPool;

use trader_domain::{Candle, TimeFrame};
use trader_infra::ports::CandleRepository;
use trader_infra::repositories::SqlxCandleRepository;

#[sqlx::test(migrations = "src/db/migrations")]
async fn save_and_retrieve_candles(pool: PgPool) {
    let repo = SqlxCandleRepository::new(pool);

    let candle = Candle::new(
        "SPY",
        TimeFrame::H1,
        Utc::now(),
        Decimal::from(400),
        Decimal::from(405),
        Decimal::from(399),
        Decimal::from(403),
        Decimal::from(1000),
    )
    .expect("candle válido");

    let inserted = repo
        .save(std::slice::from_ref(&candle))
        .await
        .expect("salvar candle");
    assert_eq!(inserted, 1);

    let from = candle.timestamp - chrono::Duration::hours(1);
    let to = candle.timestamp + chrono::Duration::hours(1);
    let retrieved = repo
        .get_range("SPY", TimeFrame::H1, from, to)
        .await
        .expect("buscar candles");

    assert_eq!(retrieved.len(), 1);
    assert_eq!(retrieved[0].symbol, "SPY");
    assert_eq!(retrieved[0].close, Decimal::from(403));
}

#[sqlx::test(migrations = "src/db/migrations")]
async fn deduplicates_candles(pool: PgPool) {
    let repo = SqlxCandleRepository::new(pool);

    let candle = Candle::new(
        "SPY",
        TimeFrame::H1,
        Utc::now(),
        Decimal::from(400),
        Decimal::from(405),
        Decimal::from(399),
        Decimal::from(403),
        Decimal::from(1000),
    )
    .expect("candle válido");

    let first = repo
        .save(std::slice::from_ref(&candle))
        .await
        .expect("primeira inserção");
    assert_eq!(first, 1, "primeira inserção deve inserir 1 candle");

    let second = repo
        .save(std::slice::from_ref(&candle))
        .await
        .expect("segunda inserção");
    assert_eq!(
        second, 0,
        "candles são imutáveis; segunda inserção idêntica deve retornar 0"
    );

    let from = candle.timestamp - chrono::Duration::hours(1);
    let to = candle.timestamp + chrono::Duration::hours(1);
    let retrieved = repo
        .get_range("SPY", TimeFrame::H1, from, to)
        .await
        .unwrap();

    assert_eq!(retrieved.len(), 1);
}
