//! Inicialização de tracing estruturado.

use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

/// Inicializa o logging com nível e formato configuráveis.
pub fn init_logging(level: &str, format: &str) {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));

    match format {
        "json" => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt::layer().json().with_span_events(FmtSpan::CLOSE))
                .init();
        }
        _ => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt::layer().pretty().with_span_events(FmtSpan::CLOSE))
                .init();
        }
    }
}
