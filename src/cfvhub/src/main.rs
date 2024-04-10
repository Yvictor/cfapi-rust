fn main() {
    println!("Hello, world!");
}
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

// fn main() {
//     // tracing_subscriber::fmt::init();
//     let subscriber = tracing_subscriber::fmt()
//         .compact()
//         .with_line_number(true)
//         .with_thread_ids(true)
//         .with_span_events(
//             tracing_subscriber::fmt::format::FmtSpan::ENTER
//                 | tracing_subscriber::fmt::format::FmtSpan::CLOSE,
//         )
//         // .with_target(false)
//         .with_max_level(Level::DEBUG)
//         .finish();
//     // .init();
//     tracing::subscriber::set_global_default(subscriber).unwrap();
//     info!("Hello, world! - C++ math should say 12={}", 12);

//     let user_event_handler = Box::new(MyUserEventHandler);
//     let config = cfapi::api::CFAPIConfig::new(
//         "sample".to_string(),
//         "1.0".to_string(),
//         false,
//         "cfapilog".to_string(),
//         "External".to_string(),
//         "SINOPACNB".to_string(),
//         "s1nopac".to_string(),
//     );
//     // let mut api = CFAPI::new(config, Some(user_event_handler));
//     let mut api = CFAPI::new(config, vec![user_event_handler], vec![], vec![]);
//     api.set_session_config(10);
//     api.set_host_config("216.221.213.14:7022", false, true);
//     api.start();
//     api.request("533", "AAPL", Commands::QUERYSNAPANDSUBSCRIBE);
//     api.request("534", "{^V}", Commands::QUERYSNAPANDSUBSCRIBEWILDCARD);
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

//     std::thread::sleep(std::time::Duration::from_secs(30 * 60));
//     // let mut session = api.pin_mut().getSession();
//     // let mut session_config = session.getSessionConfig();//.pin_mut().getSessionConfig();

//     // api
//     // ffi::cfapi::APIFactory::createSession(obj.as_mut(), &user_name, &password, &user_event_handler);
//     // obj.as_mut().createSession(&user_name, &password, &user_event_handler);
//     // obj.as_mut();

//     // ffi::cfapi::APIFactory::initialize(&app_name, &app_version, true, &log_filename);
//     // let mut instance = ffi::cfapi::APIFactory::getInstance();
//     // instance.as_mut().unwrap().initialize("sample", "1.0.0", true, "cfapilog");
//     // ffi::print_value(123);
//     // println!("Hello, world! - C++ math should say 12={}", ffi::DoMath(4));
//     // let mut goat = ffi::Goat::new().within_box();
//     // goat.as_mut().add_a_horn();
//     // goat.as_mut().add_a_horn();
//     // assert_eq!(
//     //     goat.describe().as_ref().unwrap().to_string_lossy(),
//     //     "This goat has 2 horns."
//     // );
//     // assert_eq!(ffi::do_math(12, 13), 25);
//     // print!("do_math: {}\n", ffi::do_math(20, 30));
//     // let mut goat = ffi::Goat::new().within_unique_ptr(); // returns a cxx::UniquePtr, i.e. a std::unique_ptr
//     // goat.pin_mut().add_a_horn();
//     // goat.pin_mut().add_a_horn();
//     // assert_eq!(goat.describe().as_ref().unwrap().to_string_lossy(), "This goat has 2 horns.");
// }
