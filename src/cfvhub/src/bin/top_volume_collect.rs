use cfapi::api::{CFAPIConfig, ConnectionConfig, SessionConfig, CFAPI};
use cfapi::binding::{Commands, MessageEvent};
use cfapi::event_reader::{EventReader, EventReaderSerConfig};
use cfapi::message_event::MessageEventHandlerExt;
use cfapi::value::CFValue;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{info, Level};

#[derive(Debug, Default, Clone)]
struct SymbolVolume {
    symbol: String,
    total_volume: u64,
    last_volume: u64,
    total_amount: f64,
    last_price: f64,
    messages: u64,
}

struct TopVolumeHandler {
    state: Arc<Mutex<HashMap<String, SymbolVolume>>>,
    messages: AtomicU64,
    reader_config: EventReaderSerConfig,
}

impl TopVolumeHandler {
    fn new(state: Arc<Mutex<HashMap<String, SymbolVolume>>>) -> Self {
        Self {
            state,
            messages: AtomicU64::new(0),
            reader_config: EventReaderSerConfig::default(),
        }
    }
}

impl MessageEventHandlerExt for TopVolumeHandler {
    fn on_message_event(&self, event: &MessageEvent) {
        if event.getSource() != autocxx::c_int(533) {
            return;
        }
        let symbol = event.getSymbol().to_string();
        if symbol.is_empty() {
            return;
        }

        self.messages.fetch_add(1, Ordering::Relaxed);
        let mut reader = EventReader::new(event, &self.reader_config);
        let mut total_volume = None;
        let mut last_volume = None;
        let mut total_amount = None;
        let mut last_price = None;
        for (token, value) in reader.iter_with_token_number() {
            match token {
                463 => total_volume = value_u64(value),
                448 | 9 => last_volume = value_u64(value),
                460 => total_amount = value_f64(value),
                8 | 447 => last_price = value_f64(value),
                _ => {}
            }
        }

        if total_volume.is_none() && last_volume.is_none() && total_amount.is_none() {
            return;
        }

        let mut state = self.state.lock().unwrap();
        let entry = state.entry(symbol.clone()).or_insert_with(|| SymbolVolume {
            symbol,
            ..SymbolVolume::default()
        });
        entry.messages = entry.messages.saturating_add(1);
        if let Some(value) = total_volume {
            entry.total_volume = entry.total_volume.max(value);
        }
        if let Some(value) = last_volume {
            entry.last_volume = value;
            if entry.total_volume == 0 {
                entry.total_volume = value;
            }
        }
        if let Some(value) = total_amount {
            entry.total_amount = entry.total_amount.max(value);
        }
        if let Some(value) = last_price {
            entry.last_price = value;
        }
    }
}

fn main() {
    dotenvy::dotenv().ok();
    let subscriber = tracing_subscriber::fmt()
        .compact()
        .with_thread_ids(true)
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).ok();

    let seconds = arg_or_env_u64(1, "CFVHUB_COLLECT_SECONDS", 240);
    let limit = arg_or_env_usize(2, "CFVHUB_TOP_LIMIT", 5_000);
    let output = std::env::args()
        .nth(3)
        .or_else(|| dotenvy::var("CFVHUB_TOP_OUTPUT").ok())
        .unwrap_or_else(|| "/tmp/cfvhub_top5000_symbols.txt".to_string());

    let state = Arc::new(Mutex::new(HashMap::new()));
    let handler = TopVolumeHandler::new(Arc::clone(&state));
    let mut api = build_api(handler);
    api.request("533", "{^}", Commands::QUERYSNAPANDSUBSCRIBEWILDCARD);

    let started = Instant::now();
    let mut last_count = 0u64;
    while started.elapsed() < Duration::from_secs(seconds) {
        std::thread::sleep(Duration::from_secs(10));
        let snapshot_len = state.lock().unwrap().len();
        let current_count = snapshot_len as u64;
        info!(
            elapsed_sec = started.elapsed().as_secs(),
            tracked_symbols = snapshot_len,
            new_symbols = current_count.saturating_sub(last_count),
            "collecting top volume symbols"
        );
        last_count = current_count;
    }

    let mut rows = state.lock().unwrap().values().cloned().collect::<Vec<_>>();
    rows.sort_by(|a, b| {
        b.total_volume
            .cmp(&a.total_volume)
            .then_with(|| b.total_amount.total_cmp(&a.total_amount))
            .then_with(|| a.symbol.cmp(&b.symbol))
    });
    let take = limit.min(rows.len());

    let mut file = File::create(&output).unwrap();
    for row in rows.iter().take(take) {
        writeln!(file, "{}", row.symbol).unwrap();
    }

    let csv_output = format!("{}.csv", output);
    let mut csv = File::create(&csv_output).unwrap();
    writeln!(
        csv,
        "rank,symbol,total_volume,last_volume,total_amount,last_price,messages"
    )
    .unwrap();
    for (rank, row) in rows.iter().take(take).enumerate() {
        writeln!(
            csv,
            "{},{},{},{},{:.4},{:.10},{}",
            rank + 1,
            row.symbol,
            row.total_volume,
            row.last_volume,
            row.total_amount,
            row.last_price,
            row.messages
        )
        .unwrap();
    }

    info!(
        output,
        csv_output,
        symbols_written = take,
        symbols_tracked = rows.len(),
        "top volume collection complete"
    );
}

fn build_api(handler: TopVolumeHandler) -> CFAPI {
    let cfapi_host =
        dotenvy::var("CFAPI_HOST").unwrap_or_else(|_| "216.221.209.61:7022".to_string());
    let cfapi_user = dotenvy::var("CFAPI_USER").unwrap_or_else(|_| "SINOCANNED".to_string());
    let cfapi_pass = dotenvy::var("CFAPI_PASS").expect("CFAPI_PASS must be set");

    let config = CFAPIConfig::default()
        .with_app_name("CFVHUB-TOP-VOLUME")
        .with_app_version("1.0")
        .with_username(&cfapi_user)
        .with_password(&cfapi_pass)
        .with_statistics_interval(60);

    let session_config = SessionConfig::default()
        .with_multi_threaded_api_connections(true)
        .with_max_csp_threads(env_i64("CFAPI_MAX_CSP_THREADS", 4))
        .with_max_user_threads(env_i64("CFAPI_MAX_USER_THREADS", 1))
        .with_queue_depth_threshold_percent(5);

    let connection_config = ConnectionConfig::default().with_queue_size(128);
    let mut api = CFAPI::new(config, vec![], vec![], vec![Box::new(handler)], vec![]);
    api.set_session_config(&session_config);
    api.set_connection_config(&cfapi_host, &connection_config);
    api.set_connection_config("216.221.209.62:7022", &connection_config);
    api.start();
    api
}

fn value_u64(value: CFValue) -> Option<u64> {
    match value {
        CFValue::Int(value) => value.try_into().ok(),
        CFValue::Double(value) | CFValue::Datetime(value) => {
            if value.is_finite() && value >= 0.0 {
                Some(value.round() as u64)
            } else {
                None
            }
        }
        CFValue::String(value) => value.parse().ok(),
        CFValue::Unknown => None,
    }
}

fn value_f64(value: CFValue) -> Option<f64> {
    match value {
        CFValue::Double(value) | CFValue::Datetime(value) => Some(value),
        CFValue::Int(value) => Some(value as f64),
        CFValue::String(value) => value.parse().ok(),
        CFValue::Unknown => None,
    }
}

fn env_i64(name: &str, default: i64) -> i64 {
    dotenvy::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn arg_or_env_u64(index: usize, env: &str, default: u64) -> u64 {
    std::env::args()
        .nth(index)
        .and_then(|value| value.parse().ok())
        .or_else(|| dotenvy::var(env).ok().and_then(|value| value.parse().ok()))
        .unwrap_or(default)
}

fn arg_or_env_usize(index: usize, env: &str, default: usize) -> usize {
    std::env::args()
        .nth(index)
        .and_then(|value| value.parse().ok())
        .or_else(|| dotenvy::var(env).ok().and_then(|value| value.parse().ok()))
        .unwrap_or(default)
}
