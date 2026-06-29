use cfapi::api::{CFAPIConfig, ConnectionConfig, SessionConfig, CFAPI};
use cfapi::binding::Commands;
use cfvhub::convertor::stateless_map::BTreeMapConvertor;
use cfvhub::formater::JsonFormater;
use cfvhub::pipe_thread_local::PipeThreadLocalMessageHandler;
use cfvhub::sink::DoNothingSink;
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

fn main() {
    dotenvy::dotenv().ok();
    let args = Args::parse();
    // let cfapi_host = dotenvy::var("CFAPI_HOST").unwrap_or("216.221.213.14:7022".to_string());
    let cfapi_host = dotenvy::var("CFAPI_HOST").unwrap_or("216.221.209.61:7022".to_string());
    let cfapi_user = dotenvy::var("CFAPI_USER").unwrap_or("SINOCANNED".to_string());
    let cfapi_pass = dotenvy::var("CFAPI_PASS").expect("CFAPI_PASS must be set");
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
    let pipe_thread_local_message_handler: PipeThreadLocalMessageHandler<
        // NasdaqBasicConvertorV1,
        BTreeMapConvertor,
        JsonFormater,
        // MessagePackFormater,
        // SolaceSink,
        // DiskSink,
        DoNothingSink,
    > = PipeThreadLocalMessageHandler::new(
        BTreeMapConvertor::default(),
        // NasdaqBasicConvertorV1::default(),
    );
    pipe_thread_local_message_handler.exec_metrics_loop_th();
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
        .with_queue_depth_threshold_percent(5);
    let main_connection_config = ConnectionConfig::default().with_queue_size(128);
    // let backup_connection_config = ConnectionConfig::default().with_backup(true);
    let mut api = CFAPI::new(
        config,
        vec![],
        vec![],
        // vec![Box::new(pipe_message_handler)],
        vec![Box::new(pipe_thread_local_message_handler)],
        vec![],
    );
    api.set_session_config(&session_config);
    for host in cfapi_hosts(&cfapi_host) {
        info!(host, "configure CFAPI host");
        api.set_connection_config(&host, &main_connection_config);
    }
    // api.set_connection_config("216.221.213.14:7022", &backup_connection_config);
    api.start();
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
    if dotenvy::var("CFVHUB_WILDCARD").ok().as_deref() == Some("1") {
        api.request(
            "533",
            &format!("{{^{}}}", args.subscribe_pattern),
            Commands::QUERYSNAPANDSUBSCRIBEWILDCARD,
        );
    } else {
        api.request("533", "AAPL", Commands::QUERYSNAPANDSUBSCRIBE);
        api.request("533", "NVDA", Commands::QUERYSNAPANDSUBSCRIBE);
        // api.request("533", "AAPL", Commands::SUBSCRIBE);
        // api.request("533", "AAPL", Commands::QUERYSNAP);
        api.request("534", "NKE", Commands::QUERYSNAPANDSUBSCRIBE);
    }
    // api.request("533", "{^A}", Commands::QUERYSNAPANDSUBSCRIBEWILDCARD);
    // api.request("533", "*", Commands::QUERYSNAPANDSUBSCRIBEWILDCARD);
    // api.request("533", "NVDA", Commands::QUERYSNAPANDSUBSCRIBE);
    // api.request("533", "TLSA", Commands::QUERYSNAPANDSUBSCRIBE);
    std::thread::sleep(std::time::Duration::from_secs(3 * 24 * 60 * 60));
    // std::thread::sleep(std::time::Duration::from_secs(90));
    minitrace::flush();
}
