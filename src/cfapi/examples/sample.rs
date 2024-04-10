use cfapi::api::{CFAPIConfig, CFAPI};
use cfapi::binding::Commands;
use tracing::Level;
use tracing_subscriber;

fn main() {
    let subscriber = tracing_subscriber::fmt()
        .compact()
        .with_line_number(true)
        .with_thread_ids(true)
        .with_span_events(
            tracing_subscriber::fmt::format::FmtSpan::ENTER
                | tracing_subscriber::fmt::format::FmtSpan::CLOSE,
        )
        // .with_target(false)
        .with_max_level(Level::DEBUG)
        .finish();
    // .init();
    tracing::subscriber::set_global_default(subscriber).unwrap();

    let mut api = CFAPI::new(
        CFAPIConfig::new(
            "sample".to_string(),
            "1.0".to_string(),
            false,
            "cfapilog".to_string(),
            "External".to_string(),
            "".to_string(),
            "".to_string(),
        ),
        vec![],
        vec![],
        vec![],
    );
    api.set_session_config(10);
    api.set_host_config("216.221.213.14:7022", false, true);
    api.start();
    api.request("533", "AAPL", Commands::QUERYSNAPANDSUBSCRIBE);
    // api.request("534", "{^V}", Commands::QUERYSNAPANDSUBSCRIBEWILDCARD);
    std::thread::sleep(std::time::Duration::from_secs(30 * 60));
}
