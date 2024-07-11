use cfapi::api::{CFAPIConfig, ConnectionConfig, SessionConfig, CFAPI};
use cfapi::binding::Commands;
use cfvhub::convertor::nasdaq_basic::NasdaqBasicConvertorV1;
use cfvhub::formater::{JsonFormater, MessagePackFormater};
use cfvhub::pipe::PipeMessageHandler;
use cfvhub::pipe_queue::PipeQueueMessageHandler;
use cfvhub::sink::{ConsoleSink, DiskSink, DoNothingSink, SolaceSink};
use clap::Parser;
use tracing::{info, Level};
use tracing_subscriber;

#[derive(Parser, Debug)]
#[command(version, author, about)]
struct Args {
    // exec mode
    #[arg(short, long, default_value_t = 0)]
    mode: u32,
    // sub example A or B, or A..Z
    #[arg(short, long, default_value_t = String::from("A"))]
    sub: String,
    // sink thread number
    #[arg(short = 't', long, default_value_t = 2)]
    sink_thread: usize,
}

fn main() {
    let args = Args::parse();

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
    let pipe_queue_message_handler: PipeQueueMessageHandler<
        NasdaqBasicConvertorV1,
        MessagePackFormater,
        SolaceSink,
    > = PipeQueueMessageHandler::new(
        NasdaqBasicConvertorV1::default(),
        // JsonFormater {},
        // MessagePackFormater {},
        // DiskSink::new("record.json".into()).unwrap(),
        // DoNothingSink {},
        // ConsoleSink {},
        1024,
        args.sink_thread,
    );
    pipe_queue_message_handler.exec_loop_th();
    let app_name = format!("CFVHUB-{}", args.sub);
    let config = CFAPIConfig::default()
        .with_app_name(&app_name)
        .with_app_version("1.0")
        .with_username("SINOPACNB")
        .with_password("s1nopac")
        .with_statistics_interval(60);
    let session_config = SessionConfig::default()
        .with_multi_threaded_api_connections(true)
        .with_max_csp_threads(12)
        .with_max_user_threads(12)
        .with_queue_depth_threshold_percent(5);
    let main_connection_config = ConnectionConfig::default();
    // let backup_connection_config = ConnectionConfig::default().with_backup(true);
    let mut api = CFAPI::new(
        config,
        vec![],
        vec![],
        // vec![Box::new(pipe_message_handler)],
        vec![Box::new(pipe_queue_message_handler)],
        vec![],
    );
    api.set_session_config(&session_config);
    api.set_connection_config("216.221.213.14:7022", &main_connection_config);
    // api.set_connection_config("216.221.213.14:7022", &backup_connection_config);
    api.start();
    if args.sub.chars().count() > 1 {
        let start_char = args.sub.chars().nth(0).unwrap();
        let end_char = args.sub.chars().last().unwrap();
        for a in start_char..=end_char {
            api.request(
                "533",
                &format!("{{^{}}}", a),
                Commands::QUERYSNAPANDSUBSCRIBEWILDCARD,
            );
        }
    } else {
        api.request(
            "533",
            &format!("{{^{}}}", args.sub),
            Commands::QUERYSNAPANDSUBSCRIBEWILDCARD,
        );
    }
    // api.request("533", "AAPL", Commands::QUERYSNAPANDSUBSCRIBE);
    // api.request("533", "NVDA", Commands::QUERYSNAPANDSUBSCRIBE);
    // api.request("533", "{^A}", Commands::QUERYSNAPANDSUBSCRIBEWILDCARD);
    // api.request("533", "{^B}", Commands::QUERYSNAPANDSUBSCRIBEWILDCARD);
    // api.request("533", "{^C}", Commands::QUERYSNAPANDSUBSCRIBEWILDCARD);
    // api.request("533", "*", Commands::QUERYSNAPANDSUBSCRIBEWILDCARD);
    // api.request("533", "NVDA", Commands::QUERYSNAPANDSUBSCRIBE);
    // api.request("533", "TLSA", Commands::QUERYSNAPANDSUBSCRIBE);
    std::thread::sleep(std::time::Duration::from_secs(12 * 60 * 60));
}
