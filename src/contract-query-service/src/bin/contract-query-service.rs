use contract_query_service::runtime::{run, RuntimeConfig, RuntimeError};

#[tokio::main]
async fn main() {
    let result = match RuntimeConfig::from_env() {
        Ok(config) => run(config).await,
        Err(error) => Err(RuntimeError::from(error)),
    };
    if let Err(error) = result {
        eprintln!("contract query service failed: {error}");
        std::process::exit(1);
    }
}
