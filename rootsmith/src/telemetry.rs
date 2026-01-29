use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Initialize telemetry with tracing and logging.
/// OpenTelemetry tracing is integrated via the tracing framework.
pub fn init() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rootsmith=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}
