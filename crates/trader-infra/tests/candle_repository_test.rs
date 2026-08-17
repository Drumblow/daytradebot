use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::PgPool;

use trader_domain::{Candle, CandleRepository, TimeFrame};
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

/// Auto-reparo de barras degeneradas: uma linha flat (1 print, high==low)
/// é substituída quando a versão consolidada chega; uma linha boa NUNCA é
/// sobrescrita (imutabilidade preservada). Cobre o mecanismo de reparo do
/// feed degradado da VM (ver docs/reports/validacao-live-vs-backtest-2026-08-07_a_08-14.md).
#[sqlx::test(migrations = "src/db/migrations")]
async fn degenerate_bar_is_repaired_when_consolidated_version_arrives(pool: PgPool) {
    let repo = SqlxCandleRepository::new(pool);
    let ts = Utc::now();

    let flat = Candle::new(
        "IWV",
        TimeFrame::M15,
        ts,
        Decimal::new(44198, 2),
        Decimal::new(44198, 2),
        Decimal::new(44198, 2),
        Decimal::new(44198, 2),
        Decimal::ZERO,
    )
    .expect("barra flat");
    repo.save(std::slice::from_ref(&flat)).await.unwrap();

    // Versão consolidada chega depois: substitui a flat.
    let rich = Candle::new(
        "IWV",
        TimeFrame::M15,
        ts,
        Decimal::new(44170, 2),
        Decimal::new(44199, 2),
        Decimal::new(44167, 2),
        Decimal::new(44190, 2),
        Decimal::from(649),
    )
    .expect("barra consolidada");
    let updated = repo.save(std::slice::from_ref(&rich)).await.unwrap();
    assert_eq!(updated, 1, "linha degenerada deve ser reparada");

    let from = ts - chrono::Duration::hours(1);
    let to = ts + chrono::Duration::hours(1);
    let retrieved = repo
        .get_range("IWV", TimeFrame::M15, from, to)
        .await
        .unwrap();
    assert_eq!(retrieved.len(), 1);
    assert_eq!(retrieved[0].low, Decimal::new(44167, 2));
    assert_eq!(retrieved[0].volume, Decimal::from(649));

    // Uma vez boa, a linha é imutável: nova versão flat NÃO sobrescreve.
    let not_updated = repo.save(std::slice::from_ref(&flat)).await.unwrap();
    assert_eq!(not_updated, 0, "linha boa é imutável");
    let retrieved = repo
        .get_range("IWV", TimeFrame::M15, from, to)
        .await
        .unwrap();
    assert_eq!(retrieved[0].low, Decimal::new(44167, 2));
}
