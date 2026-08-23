use contract_query_service::runtime::{run, RuntimeConfig, RuntimeError};
use tracing_subscriber::{filter::Targets, layer::SubscriberExt, util::SubscriberInitExt};

const DEFAULT_LOG_FILTER: &str = "warn,contract_query_service=info";

fn init_tracing() {
    let (filter, invalid_filter) = match std::env::var("RUST_LOG") {
        Ok(value) => match value.parse::<Targets>() {
            Ok(filter) => (filter, false),
            Err(_) => (default_log_filter(), true),
        },
        Err(_) => (default_log_filter(), false),
    };

    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .compact()
                .with_thread_ids(true),
        )
        .init();

    if invalid_filter {
        tracing::warn!("invalid RUST_LOG filter; using the default filter");
    }
}

fn default_log_filter() -> Targets {
    DEFAULT_LOG_FILTER
        .parse()
        .expect("the built-in tracing filter is valid")
}

#[tokio::main]
async fn main() {
    init_tracing();
    let result = match RuntimeConfig::from_env() {
        Ok(config) => run(config).await,
        Err(error) => Err(RuntimeError::from(error)),
    };
    if let Err(error) = result {
        eprintln!("contract query service failed: {error}");
        std::process::exit(1);
    }
}
