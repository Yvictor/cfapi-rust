use cfapi::api::{CFAPIConfig, ConnectionConfig, SessionConfig, CFAPI};
use cfapi::binding::Commands;
use cfapi::message_event::{DefaultMessageEventHandler, MessageEventHandlerExt};
use cfvhub::convertor::nasdaq_solace::NasdaqSolaceConvertorV1;
use cfvhub::formater::MessagePackFormater;
use cfvhub::pipe_thread_local::PipeThreadLocalMessageHandler;
#[cfg(not(feature = "solace"))]
use cfvhub::sink::SolaceConsoleSink as OutputSink;
#[cfg(feature = "solace")]
use cfvhub::sink::SolaceSink as OutputSink;
// SolaceSink
use clap::Parser;
use tracing::{info, Level};
use tracing_subscriber;

#[derive(Parser, Debug)]
#[command(version, author, about)]
struct Args {
    // exec mode
    #[arg(short, long, default_value_t = 0)]
    mode: u32,
    // subscribe pattern example A or B, or A..Z
    #[arg(short, long, default_value_t = String::from("A"))]
    subscribe_pattern: String,
    // CFAPI user thread count fallback when CFAPI_MAX_USER_THREADS is not set
    #[arg(short = 't', long, default_value_t = 2)]
    sink_thread: usize,
}

fn cfapi_hosts(default_host: &str) -> Vec<String> {
    if let Ok(hosts) = dotenvy::var("CFAPI_HOSTS") {
        return hosts
            .split(',')
            .map(str::trim)
            .filter(|host| !host.is_empty())
            .map(ToOwned::to_owned)
            .collect();
    }

    let Some(port_end) = dotenvy::var("CFAPI_PORT_END")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
    else {
        let mut hosts = vec![default_host.to_owned()];
        if let Some((_, port)) = default_host.rsplit_once(':') {
            let backup = format!("216.221.209.62:{}", port);
            if !hosts.iter().any(|host| host == &backup) {
                hosts.push(backup);
            }
        }
        return hosts;
    };

    let port_start = dotenvy::var("CFAPI_PORT_START")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .or_else(|| {
            default_host
                .rsplit_once(':')
                .and_then(|(_, port)| port.parse::<u16>().ok())
        })
        .unwrap_or(7022);
    let ips =
        dotenvy::var("CFAPI_IPS").unwrap_or_else(|_| "216.221.209.61,216.221.209.62".to_string());

    let mut hosts = std::collections::BTreeSet::new();
    for ip in ips.split(',').map(str::trim).filter(|ip| !ip.is_empty()) {
        for port in port_start..=port_end {
            hosts.insert(format!("{}:{}", ip, port));
        }
    }
    hosts.into_iter().collect()
}

fn load_symbols(path: &str, limit: usize) -> Vec<String> {
    let content = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read symbol file {}: {}", path, error));
    let mut symbols = Vec::with_capacity(limit);
    for line in content.lines() {
        let line = line.split('#').next().unwrap_or("");
        for symbol in line
            .split(|ch: char| ch == ',' || ch == ';' || ch.is_whitespace())
            .map(str::trim)
            .filter(|symbol| !symbol.is_empty())
        {
            symbols.push(symbol.to_ascii_uppercase());
            if symbols.len() == limit {
                return symbols;
            }
        }
    }
    symbols
}

fn main() {
    dotenvy::dotenv().ok();
    let args = Args::parse();
    // let cfapi_host = dotenvy::var("CFAPI_HOST").unwrap_or("216.221.213.14:7022".to_string());
    let cfapi_host = dotenvy::var("CFAPI_HOST").unwrap_or("216.221.209.61:7022".to_string());
    let cfapi_user = dotenvy::var("CFAPI_USER").unwrap_or("SINOCANNED".to_string());
    let cfapi_pass = dotenvy::var("CFAPI_PASS").expect("CFAPI_PASS must be set");
    let symbol_file = dotenvy::var("CFVHUB_SYMBOL_FILE").ok();
    let symbols = symbol_file
        .as_deref()
        .map(|path| load_symbols(path, 5_000))
        .unwrap_or_default();
    let source_id = dotenvy::var("CFVHUB_SOURCE_ID").unwrap_or_else(|_| "533".to_string());
    // let reporter =
    //     // minitrace_jaeger::JaegerReporter::new("128.110.5.124:6831".parse().unwrap(), "cfvhub")
    //     //     .unwrap();
    // minitrace::set_reporter(reporter, Config::default());

    let subscriber = tracing_subscriber::fmt()
        .compact()
        // .with_line_number(true)
        .with_thread_ids(true)
        .with_file(false)
        .with_span_events(
            tracing_subscriber::fmt::format::FmtSpan::ENTER
                | tracing_subscriber::fmt::format::FmtSpan::CLOSE,
        )
        // .with_target(false)
        .with_max_level(Level::INFO)
        .finish();
    // .init();
    tracing::subscriber::set_global_default(subscriber).unwrap();

    info!("CFVHUB Start mode: {}", args.mode);
    let source_port_details_mode = args.mode == 9
        || dotenvy::var("CFAPI_GET_SOURCE_PORT_DETAILS")
            .ok()
            .as_deref()
            == Some("1");
    if source_port_details_mode {
        info!("source port details diagnostic mode enabled");
    }
    let message_handlers: Vec<Box<dyn MessageEventHandlerExt + Send + Sync>> =
        if source_port_details_mode {
            vec![Box::new(DefaultMessageEventHandler::default())
                as Box<dyn MessageEventHandlerExt + Send + Sync>]
        } else {
            let pipe_thread_local_message_handler: PipeThreadLocalMessageHandler<
                NasdaqSolaceConvertorV1,
                MessagePackFormater,
                OutputSink,
            > = PipeThreadLocalMessageHandler::new(NasdaqSolaceConvertorV1::default());
            pipe_thread_local_message_handler.exec_metrics_loop_th();
            vec![Box::new(pipe_thread_local_message_handler)
                as Box<dyn MessageEventHandlerExt + Send + Sync>]
        };
    let app_name = format!("CFVHUB-{}", args.subscribe_pattern);
    let config = CFAPIConfig::default()
        .with_app_name(&app_name)
        .with_app_version("1.0")
        // .with_username("SINOPACNB")
        // .with_password("s1nopac")
        .with_username(&cfapi_user)
        .with_password(&cfapi_pass)
        .with_statistics_interval(60);
    let session_config = SessionConfig::default()
        .with_multi_threaded_api_connections(true)
        .with_max_csp_threads(
            dotenvy::var("CFAPI_MAX_CSP_THREADS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(4),
        )
        .with_max_user_threads(
            dotenvy::var("CFAPI_MAX_USER_THREADS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(args.sink_thread as i64),
        )
        .with_watchlist(!symbols.is_empty())
        .with_max_watchlist_size(5_000)
        .with_queue_depth_threshold_percent(5);
    let connection_read_timeout = dotenvy::var("CFAPI_READ_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(15);
    let connection_queue_size = dotenvy::var("CFAPI_CONNECTION_QUEUE_MB")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(128);
    let main_connection_config = ConnectionConfig::default()
        .with_read_timeout(connection_read_timeout)
        .with_queue_size(connection_queue_size);
    // let backup_connection_config = ConnectionConfig::default().with_backup(true);
    let mut api = CFAPI::new(
        config,
        vec![],
        vec![],
        // vec![Box::new(pipe_message_handler)],
        message_handlers,
        vec![],
    );
    api.set_session_config(&session_config);
    api.set_global_connection_config(&main_connection_config);
    info!(
        read_timeout_secs = connection_read_timeout,
        queue_size_mb = connection_queue_size,
        "configure CFAPI global connection defaults"
    );
    for host in cfapi_hosts(&cfapi_host) {
        info!(host, "configure CFAPI host");
        api.set_connection_config(&host, &main_connection_config);
    }
    // api.set_connection_config("216.221.213.14:7022", &backup_connection_config);
    api.start();
    if source_port_details_mode {
        let tag = api.command(Commands::GETSOURCEPORTDETAILS);
        info!(tag, "sent GETSOURCEPORTDETAILS");
    } else {
        // if args.subscribe_pattern.chars().count() > 1 {
        //     let start_char = args.subscribe_pattern.chars().nth(0).unwrap();
        //     let end_char = args.subscribe_pattern.chars().last().unwrap();
        //     for a in start_char..=end_char {
        //         api.request(
        //             "533",
        //             &format!("{{^{}}}", a),
        //             Commands::QUERYSNAPANDSUBSCRIBEWILDCARD,
        //         );
        //     }
        // } else {
        //     api.request(
        //         "533",
        //         &format!("{{^{}}}", args.subscribe_pattern),
        //         Commands::QUERYSNAPANDSUBSCRIBEWILDCARD,
        //     );
        // }
        if !symbols.is_empty() {
            info!(
                source_id = source_id.as_str(),
                symbol_count = symbols.len(),
                symbol_file = symbol_file.as_deref().unwrap_or(""),
                "subscribe symbol file"
            );
            for symbol in &symbols {
                api.request(&source_id, symbol, Commands::QUERYSNAPANDSUBSCRIBE);
            }
        } else if dotenvy::var("CFVHUB_WILDCARD").ok().as_deref() == Some("1") {
            api.request(
                &source_id,
                &format!("{{^{}}}", args.subscribe_pattern),
                Commands::QUERYSNAPANDSUBSCRIBEWILDCARD,
            );
        } else {
            api.request(&source_id, "AAPL", Commands::QUERYSNAPANDSUBSCRIBE);
            api.request(&source_id, "NVDA", Commands::QUERYSNAPANDSUBSCRIBE);
            // api.request(&source_id, "AAPL", Commands::SUBSCRIBE);
            // api.request(&source_id, "AAPL", Commands::QUERYSNAP);
            api.request("534", "NKE", Commands::QUERYSNAPANDSUBSCRIBE);
        }
    }
    // api.request("533", "{^A}", Commands::QUERYSNAPANDSUBSCRIBEWILDCARD);
    // api.request("533", "*", Commands::QUERYSNAPANDSUBSCRIBEWILDCARD);
    // api.request("533", "NVDA", Commands::QUERYSNAPANDSUBSCRIBE);
    // api.request("533", "TLSA", Commands::QUERYSNAPANDSUBSCRIBE);
    let run_seconds = if source_port_details_mode {
        dotenvy::var("CFAPI_SOURCE_PORT_DETAILS_WAIT_SECS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(60)
    } else {
        3 * 24 * 60 * 60
    };
    std::thread::sleep(std::time::Duration::from_secs(run_seconds));
    // std::thread::sleep(std::time::Duration::from_secs(90));
    minitrace::flush();
}
