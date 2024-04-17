use cfapi::api::{CFAPIConfig, ConnectionConfig, SessionConfig, CFAPI};
use cfapi::binding::Commands;
use cfvhub::convertor::{BTreeMapConvertor, StatefulBTreeMapConvertor};
use cfvhub::formater::JsonFormater;
use cfvhub::pipe::PipeMessageHandler;
use cfvhub::sink::DiskSink;
use tracing::Level;
use tracing_subscriber;

// use cfapi_rust::cfapi::api::CFAPI;
// use cfapi_rust::cfapi::binding::{Commands, UserEvent, UserEvent_Types};
// use cfapi_rust::cfapi::user_event::UserEventHandlerExt;
// use crossbeam_channel::unbounded;
// use cxx::let_cxx_string;
// use rsolace::solclient::{SessionProps, SolClient};
// use rsolace::solmsg::SolMsg;
// use rsolace::types::SolClientLogLevel;
// use tracing::{debug, info, Level};
// use tracing_subscriber;

// struct MyUserEventHandler;

// impl UserEventHandlerExt for MyUserEventHandler {
//     fn on_user_event(&mut self, event: &UserEvent) {
//         let event_type = event.getType();
//         match event_type {
//             UserEvent_Types::AUTHORIZATION_FAILURE => {
//                 info!("CUSTOM AUTHORIZATION_FAILURE");
//             }
//             UserEvent_Types::AUTHORIZATION_SUCCESS => {
//                 info!("CUSTOM AUTHORIZATION_SUCCESS");
//             }
//         }
//     }
// }

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

    let pipe_message_handler = PipeMessageHandler::new(
        // BTreeMapConvertor::default(),
        StatefulBTreeMapConvertor::default(),
        JsonFormater {},
        DiskSink::new("record.json".into()).unwrap(),
    );

    let config = CFAPIConfig::default()
        .with_app_name("sample")
        .with_app_version("1.0")
        .with_username("SINOPACNB")
        .with_password("s1nopac")
        .with_statistics_interval(5 * 60);
    let session_config = SessionConfig::default()
        .with_multi_threaded_api_connections(true)
        .with_max_user_threads(12);
    let main_connection_config = ConnectionConfig::default();
    // let backup_connection_config = ConnectionConfig::default().with_backup(true);
    let mut api = CFAPI::new(
        config,
        vec![],
        vec![],
        vec![Box::new(pipe_message_handler)],
        vec![],
    );
    api.set_session_config(&session_config);
    api.set_connection_config("216.221.213.14:7022", &main_connection_config);
    // api.set_connection_config("216.221.213.14:7022", &backup_connection_config);
    api.start();
    api.request("533", "AAPL", Commands::QUERYSNAPANDSUBSCRIBE);
    // api.request("533", "NVDA", Commands::QUERYSNAPANDSUBSCRIBE);
    // api.request("533", "TLSA", Commands::QUERYSNAPANDSUBSCRIBE);
    api.request("534", "{^V}", Commands::QUERYSNAPANDSUBSCRIBEWILDCARD);
    //     std::thread::spawn(move || {
    //         let solclient = SolClient::new(SolClientLogLevel::Notice);
    //         match solclient {
    //             Ok(mut solclient) => {
    //                 let session_props = SessionProps::default()
    //                     .host("128.110.5.101:55555")
    //                     .vpn("sinopac")
    //                     .username("shioaji")
    //                     .password("shioaji111");
    //                 let r = solclient.connect(session_props);
    //                 info!("connect: {:?}", r);
    //                 let event_recv = solclient.get_event_receiver();
    //                 let _th_event = std::thread::spawn(move || loop {
    //                     match event_recv.recv() {
    //                         Ok(event) => {
    //                             info!("{:?}", event);
    //                         }
    //                         Err(e) => {
    //                             tracing::error!("recv event error: {:?}", e);
    //                             break;
    //                         }
    //                     }
    //                 });
    //                 // match recviver.recv() {
    //                 //     Ok(data) => {
    //                 //         info!("recv data: {:?}", data);
    //                 //         let mut msg = SolMsg::new().unwrap();
    //                 //         msg.set_topic("api/v1/test");
    //                 //         let msgp_data = rmp_serde::to_vec_named(&data).unwrap();
    //                 //         msg.set_binary_attachment(&msgp_data);
    //                 //         // std::thread::sleep(std::time::Duration::from_secs(5));
    //                 //         let rt = solclient.send_msg(&msg);
    //                 //         info!("send msg: {:?}", rt);
    //                 //     }
    //                 //     Err(e) => {
    //                 //         tracing::error!("recv error: {:?}", e);
    //                 //     }
    //                 // }

    //                 std::thread::sleep(std::time::Duration::from_secs(30 * 60));
    //             }
    //             Err(e) => {
    //                 info!("create solclient error: {:?}", e);
    //             }
    //         }
    //     });

    std::thread::sleep(std::time::Duration::from_secs(30 * 60));
}
