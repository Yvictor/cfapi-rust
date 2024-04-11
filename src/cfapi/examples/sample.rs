use cfapi::api::{CFAPIConfig, ConnectionConfig, SessionConfig, CFAPI};
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

    let mut api = CFAPI::new(CFAPIConfig::default(), vec![], vec![], vec![], vec![]);
    let session_config = SessionConfig::default();
    let connection_config = ConnectionConfig::default();
    api.set_session_config(&session_config);
    api.set_connection_config("216.221.213.14:7022", &connection_config);
    api.start();
    api.request("533", "AAPL", Commands::QUERYSNAPANDSUBSCRIBE);
    // api.request("534", "{^V}", Commands::QUERYSNAPANDSUBSCRIBEWILDCARD);
    std::thread::sleep(std::time::Duration::from_secs(30 * 60));
}
