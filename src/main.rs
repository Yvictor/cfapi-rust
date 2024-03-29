// Use all the autocxx types which might be handy.
use autocxx::prelude::*;
// use autocxx::subclass::prelude::*;
use ahash::RandomState;
use autocxx::subclass::*;
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use cxx::let_cxx_string;
use dashmap::DashMap;
use rsolace::solclient::{SessionProps, SolClient};
use rsolace::solmsg::SolMsg;
use rsolace::types::SolClientLogLevel;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, span, Level};
use tracing_subscriber;
// use cxx::{}

// include_cpp! {
//     #include "sample.hpp"
//     safety!(unsafe_ffi)
//     generate!("print_value") // allowlist a function
//     generate!("DoMath")
//     generate!("Goat")
// }
// pub mod session {
//     autocxx::include_cpp! {
//         #include "cfapi.h"
//         name!(ffi_session)
//         safety!(unsafe_ffi)
//         generate!("cfapi::Session")
//     }
//     pub use ffi_session::*;
// }

// pub mod user {
//     autocxx::include_cpp! {
//         #include "cfapi.h"
//         name!(ffi_user)
//         safety!(unsafe_ffi)
//         generate!("cfapi::UserInfo")
//     }
//     pub use ffi_user::*;
// }

include_cpp! {
    #include "cfapi.h"
    #include "api.h"
    // #include "APIFactory.h"
    safety!(unsafe_ffi)
    generate!("cfapi::Session")
    // generate!("cfapi::APIFactory")
    generate!("APIFactoryWrap")
    generate!("GetEventReader")
    // generate!("GetDatetime")
    generate!("GetDate")
    generate!("GetTime")
    // extern_cpp_type!("cfapi::Session", crate::session::cfapi::Session)
    // generate!("cfapi::UserEventHandler")
    generate!("cfapi::SessionConfig")
    generate!("cfapi::HostConfig")
    generate!("cfapi::UserInfo")
    // extern_cpp_type!("cfapi::UserInfo", crate::user::cfapi::UserInfo)
    subclass!("cfapi::UserEventHandler", MyUserEventHandler)
    generate!("cfapi::UserEvent")
    subclass!("cfapi::SessionEventHandler", MySessionEventHandler)
    generate!("cfapi::SessionEvent")
    subclass!("cfapi::MessageEventHandler", MyMessageEventHandler)
    generate!("cfapi::MessageEvent")
    generate!("cfapi::MessageReader")
    generate!("cfapi::ValueTypes")
    generate!("cfapi::Commands")
    generate!("cfapi::DateTime")
    generate!("cfapi::Date")
    generate!("cfapi::Time")
}
use ffi::*;

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct DataSrc533 {
    symbol: String,
    ts: f64,
    ask_price: f64,
    ask_size: i64,
    bid_price: f64,
    bid_size: i64,
    price: f64,
    volume: i64,
}

#[subclass] //(superclass("cfapi::UserEventHandler"))
#[derive(Default)]
pub struct MyUserEventHandler;

impl cfapi::UserEventHandler_methods for MyUserEventHandler {
    fn onUserEvent(&mut self, event: &cfapi::UserEvent) {
        let event_type = event.getType();
        debug!("on event: {:?}", event.getRetCode());
        match event_type {
            cfapi::UserEvent_Types::AUTHORIZATION_FAILURE => {
                info!("AUTHORIZATION_FAILURE");
            }
            cfapi::UserEvent_Types::AUTHORIZATION_SUCCESS => {
                info!("AUTHORIZATION_SUCCESS");
            }
        }
    }
}

#[subclass] //(superclass("cfapi::SessionEventHandler"))
#[derive(Default)]
pub struct MySessionEventHandler;

impl cfapi::SessionEventHandler_methods for MySessionEventHandler {
    fn onSessionEvent(&mut self, event: &cfapi::SessionEvent) {
        let event_type = event.getType() as cfapi::SessionEvent_Types; // as cfapi::SessionEvent::Types
        match event_type {
            cfapi::SessionEvent_Types::CFAPI_SESSION_UNAVAILABLE => {
                info!("session unavailable");
            }
            cfapi::SessionEvent_Types::CFAPI_SESSION_ESTABLISHED => {
                info!("session established");
            }
            cfapi::SessionEvent_Types::CFAPI_SESSION_RECOVERY => {
                info!("session recovery");
            }
            cfapi::SessionEvent_Types::CFAPI_SESSION_RECOVERY_SOURCES => {
                info!("session recovery sourceID = {:?}", event.getSourceID());
            }
            cfapi::SessionEvent_Types::CFAPI_CDD_LOADED => {
                info!("cdd loaded version: {}", event.getCddVersion());
            }
            cfapi::SessionEvent_Types::CFAPI_SESSION_AVAILABLE_ALLSOURCES => {
                let source_id = event.getSourceID();
                info!("session available all sources sourceID = {:?}", source_id);
            }
            cfapi::SessionEvent_Types::CFAPI_SESSION_AVAILABLE_SOURCES => {
                let source_id = event.getSourceID();
                info!("session available sources sourceID = {:?}", source_id);
            }
            cfapi::SessionEvent_Types::CFAPI_SESSION_RECEIVE_QUEUE_ABOVE_THRESHOLD => {
                info!(
                    "session receive queue above threshold {:?}",
                    event.getQueueDepth()
                );
            }
            cfapi::SessionEvent_Types::CFAPI_SESSION_RECEIVE_QUEUE_BELOW_THRESHOLD => {
                info!(
                    "session receive queue below threshold {:?}",
                    event.getQueueDepth()
                );
            }
            cfapi::SessionEvent_Types::CFAPI_SESSION_JIT_START_CONFLATING => {
                info!("session jit start conflation");
            }
            cfapi::SessionEvent_Types::CFAPI_SESSION_JIT_STOP_CONFLATING => {
                info!("session jit stop conflation");
            }
            cfapi::SessionEvent_Types::CFAPI_SESSION_SOURCE_ADDED => {
                info!("session source added sourceID = {:?}", event.getSourceID());
            }
            cfapi::SessionEvent_Types::CFAPI_SESSION_SOURCE_REMOVED => {
                info!(
                    "session source removed sourceID = {:?}",
                    event.getSourceID()
                );
            } // _ => {
              //     info!("Ubknown event type");
              // }
        }
    }
}

#[subclass]
#[derive(Default)]
pub struct MyMessageEventHandler {
    pub maps: DashMap<String, DataSrc533, RandomState>,
    pub sender: Option<Sender<DataSrc533>>,
    pub debug: bool,
}

impl cfapi::MessageEventHandler_methods for MyMessageEventHandler {
    fn onMessageEvent(&mut self, event: &cfapi::MessageEvent) {
        // info!("onMessageEvent");
        let span = span!(Level::DEBUG, "onMessageEvent");
        let _enter = span.enter();
        let event_type = event.getType() as cfapi::MessageEvent_Types;
        match event_type {
            // cfapi::MessageEvent_Types::STATUS | cfapi::MessageEvent_Types::IMAGE_COMPLETE => {
            cfapi::MessageEvent_Types::IMAGE_PART | cfapi::MessageEvent_Types::IMAGE_COMPLETE => {
                info!(
                    "event type: {}",
                    match event_type {
                        cfapi::MessageEvent_Types::IMAGE_PART => "IMAGE_PART",
                        cfapi::MessageEvent_Types::IMAGE_COMPLETE => "IMAGE_COMPLETE",
                        cfapi::MessageEvent_Types::STATUS => "STATUS",
                        _ => "UNKNOWN",
                    }
                );
                if self.debug {
                    info!("get respones tag: {:?}", event.getTag());
                    info!(
                        "Status code={:?} ({}) for tag {}",
                        event.getStatusCode(),
                        event.getStatusString(),
                        event.getTag()
                    );
                }
                let src = event.getSource();
                if src == autocxx::c_int(533) {
                    info!("source: {:?}", src);
                    let reader = GetEventReader(event) as *mut cfapi::MessageReader;
                    let mut reader = unsafe { std::pin::Pin::new_unchecked(&mut *reader) };

                    let price = if reader.as_mut().find(autocxx::c_int(8)) {
                        reader.as_mut().getValueAsDouble()
                    } else {
                        0.0
                    };
                    let volume = if reader.as_mut().find(autocxx::c_int(9)) {
                        reader.as_mut().getValueAsInteger()
                    } else {
                        0
                    };
                    let ask_price = if reader.as_mut().find(autocxx::c_int(207)) {
                        reader.as_mut().getValueAsDouble()
                    } else {
                        0.0
                    };
                    let ask_size = if reader.as_mut().find(autocxx::c_int(791)) {
                        reader.as_mut().getValueAsInteger()
                    } else {
                        0
                    };
                    let bid_price = if reader.as_mut().find(autocxx::c_int(218)) {
                        reader.as_mut().getValueAsDouble()
                    } else {
                        0.0
                    };
                    let bid_size = if reader.as_mut().find(autocxx::c_int(790)) {
                        reader.as_mut().getValueAsInteger()
                    } else {
                        0
                    };
                    let ts = if reader.as_mut().find(autocxx::c_int(18)) {
                        reader.as_mut().getValueAsDouble()
                    } else {
                        0.0
                    };

                    let data = DataSrc533 {
                        symbol: event.getSymbol().to_string(),
                        ts: ts,
                        ask_price: ask_price,
                        ask_size: ask_size,
                        bid_price: bid_price,
                        bid_size: bid_size,
                        price: price,
                        volume: volume,
                    };
                    info!("snap data: {:?}", data);
                    let symbol = event.getSymbol().to_string();
                    self.maps.insert(symbol, data.clone());
                    debug!("map lens: {}", self.maps.len());
                    match self.sender {
                        Some(ref s) => {
                            s.send(data).unwrap();
                        }
                        None => {
                            info!("sender is none");
                        }
                    }
                    // let symbol = event.getSymbol().to_string();
                    // let v = self.maps.get(&symbol).unwrap();
                    // info!("get data: {:?}", *v);
                };
            }
            cfapi::MessageEvent_Types::UPDATE => {
                let src = event.getSource();
                let symbol = event.getSymbol();
                info!("Update Event for source: {:?} symbol: {:?}", src, symbol);
                if src == autocxx::c_int(533) {
                    let reader = GetEventReader(event) as *mut cfapi::MessageReader;
                    let mut reader = unsafe { std::pin::Pin::new_unchecked(&mut *reader) };
                    if let Some(mut data) = self.maps.get_mut(symbol.to_str().unwrap()) {
                        debug!("ref data: {:?}", *data);
                        if reader.as_mut().find(autocxx::c_int(8)) {
                            debug!("update price: {}", reader.as_mut().getValueAsDouble());
                            (*data).price = reader.as_mut().getValueAsDouble()
                        }
                        if reader.as_mut().find(autocxx::c_int(9)) {
                            debug!("update volume: {}", reader.as_mut().getValueAsInteger());
                            (*data).volume = reader.as_mut().getValueAsInteger()
                        }
                        if reader.as_mut().find(autocxx::c_int(10)) {
                            debug!("update ask price: {}", reader.as_mut().getValueAsDouble());
                            (*data).ask_price = reader.as_mut().getValueAsDouble()
                        }
                        if reader.as_mut().find(autocxx::c_int(11)) {
                            info!("update ask size: {}", reader.as_mut().getValueAsInteger());
                            (*data).ask_size = reader.as_mut().getValueAsInteger()
                        }
                        if reader.as_mut().find(autocxx::c_int(12)) {
                            debug!("update bid price: {}", reader.as_mut().getValueAsDouble());
                            (*data).bid_price = reader.as_mut().getValueAsDouble()
                        }
                        if reader.as_mut().find(autocxx::c_int(13)) {
                            info!("update bid size: {}", reader.as_mut().getValueAsInteger());
                            (*data).bid_size = reader.as_mut().getValueAsInteger()
                        }
                        if reader.as_mut().find(autocxx::c_int(14)) {
                            debug!("update price: {}", reader.as_mut().getValueAsDouble());
                            (*data).price = reader.as_mut().getValueAsDouble()
                        }
                        if reader.as_mut().find(autocxx::c_int(16)) {
                            (*data).ts = reader.as_mut().getValueAsDouble()
                        }
                        // *data = DataSrc533 {
                        //     symbol: data.symbol.clone(),
                        //     ts: if reader.as_mut().find(autocxx::c_int(16)) {
                        //         reader.as_mut().getValueAsDouble()
                        //     } else {
                        //         data.ts
                        //     },
                        //     ask_price: if reader.as_mut().find(autocxx::c_int(207)) {
                        //         reader.as_mut().getValueAsDouble()
                        //     } else {
                        //         data.ask_price
                        //     },
                        //     ask_size: if reader.as_mut().find(autocxx::c_int(791)) {
                        //         reader.as_mut().getValueAsInteger()
                        //     } else {
                        //         data.ask_size
                        //     },
                        //     bid_price: if reader.as_mut().find(autocxx::c_int(218)) {
                        //         reader.as_mut().getValueAsDouble()
                        //     } else {
                        //         data.bid_price
                        //     },
                        //     bid_size: if reader.as_mut().find(autocxx::c_int(790)) {
                        //         reader.as_mut().getValueAsInteger()
                        //     } else {
                        //         data.bid_size
                        //     },
                        //     price: if reader.as_mut().find(autocxx::c_int(8)) {
                        //         reader.as_mut().getValueAsDouble()
                        //     } else {
                        //         data.price
                        //     },
                        //     volume: 0,
                        // };
                    }
                    let data = self.maps.get(symbol.to_str().unwrap()).unwrap();
                    info!("upd data: {:?}", *data);
                }
            }
            cfapi::MessageEvent_Types::REFRESH => {
                info!("Refresh Event");
                let src = event.getSource();
                let symbol = event.getSymbol();
                if src == autocxx::c_int(533) {
                    let reader = GetEventReader(event) as *mut cfapi::MessageReader;
                    let mut reader = unsafe { std::pin::Pin::new_unchecked(&mut *reader) };

                    if let Some(mut data) = self.maps.get_mut(symbol.to_str().unwrap()) {
                        info!("data: {:?}", *data);
                        // if reader.as_mut().find(autocxx::c_int(207)) {
                        //     (*data).ask_price = reader.as_mut().getValueAsDouble()
                        // }
                        *data = DataSrc533 {
                            symbol: data.symbol.clone(),
                            ts: if reader.as_mut().find(autocxx::c_int(16)) {
                                reader.as_mut().getValueAsDouble()
                            } else {
                                data.ts
                            },
                            ask_price: if reader.as_mut().find(autocxx::c_int(207)) {
                                reader.as_mut().getValueAsDouble()
                            } else {
                                data.ask_price
                            },
                            ask_size: if reader.as_mut().find(autocxx::c_int(791)) {
                                reader.as_mut().getValueAsInteger()
                            } else {
                                data.ask_size
                            },
                            bid_price: if reader.as_mut().find(autocxx::c_int(218)) {
                                reader.as_mut().getValueAsDouble()
                            } else {
                                data.bid_price
                            },
                            bid_size: if reader.as_mut().find(autocxx::c_int(790)) {
                                reader.as_mut().getValueAsInteger()
                            } else {
                                data.bid_size
                            },
                            price: if reader.as_mut().find(autocxx::c_int(8)) {
                                reader.as_mut().getValueAsDouble()
                            } else {
                                data.price
                            },
                            volume: 0,
                        };
                    }
                    let data = self.maps.get(symbol.to_str().unwrap()).unwrap();
                    info!("get data: {:?}", *data);
                }
            }
            _ => {
                info!("event_type: {}", event_type as i32);
            }
        }
        if self.debug {
            let perm = event.getPermission();
            println!("permission: {:?}", perm);
            // let src = event.getSource();
            // println!("source: {:?}", src);

            let symbol = event.getSymbol();
            println!("symbol: {:?}", symbol);
            let reader = GetEventReader(event) as *mut cfapi::MessageReader;
            let mut reader = unsafe { std::pin::Pin::new_unchecked(&mut *reader) };
            //cfapi::MessageReader::END_OF_MESSAGE
            reader.as_mut().find(autocxx::c_int(3)); // reset to first field
            while reader.as_mut().next() != autocxx::c_int(-1) {
                match reader.as_mut().getValueType() {
                    cfapi::ValueTypes::INT64 => {
                        println!(
                            "{}({})={}",
                            reader.as_mut().getTokenName(),
                            i32::from(reader.as_mut().getTokenNumber()),
                            reader.as_mut().getValueAsInteger()
                        );
                    }
                    cfapi::ValueTypes::DOUBLE => {
                        println!(
                            "{}({})={}",
                            reader.as_mut().getTokenName(),
                            i32::from(reader.as_mut().getTokenNumber()),
                            reader.as_mut().getValueAsDouble()
                        );
                    }
                    cfapi::ValueTypes::STRING => {
                        println!(
                            "{}({})={}",
                            reader.as_mut().getTokenName(),
                            i32::from(reader.as_mut().getTokenNumber()),
                            reader.as_mut().getValueAsString()
                        );
                    }
                    cfapi::ValueTypes::DATETIME => {
                        let d = GetDate(&reader.as_ref()) as *mut cfapi::Date;
                        let t = GetTime(&reader.as_ref()) as *mut cfapi::Time;
                        let mut d = unsafe { std::pin::Pin::new_unchecked(&mut *d) };
                        let mut t = unsafe { std::pin::Pin::new_unchecked(&mut *t) };
                        let y = d.as_mut().year();
                        let m = d.as_mut().month();
                        let d = d.as_mut().day();
                        let h = t.as_mut().hour();
                        let min = t.as_mut().minute();
                        let s = t.as_mut().second();
                        let ms = (t.as_mut().millisecond() as i32 * 1000)
                            + t.as_mut().microsecond() as i32;
                        println!(
                            "{}({})=datetime {}-{:02}-{:02} {:02}:{:02}:{:02}.{:06} UTC({})",
                            reader.as_mut().getTokenName(),
                            i32::from(reader.as_mut().getTokenNumber()),
                            y,
                            m,
                            d,
                            h,
                            min,
                            s,
                            ms,
                            reader.as_mut().getValueAsDouble(),
                        );
                    }
                    _ => {
                        println!(
                            "{}({})=unknown type",
                            reader.as_mut().getTokenName(),
                            i32::from(reader.as_mut().getTokenNumber()),
                        );
                    }
                }
            }
        }

        // span.exit();
        debug!("<EXT>");
    }
}

// impl Drop for MyUserEventHandler {
//     fn drop(&mut self) {

//     }
// }

fn main() {
    // tracing_subscriber::fmt::init();
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
    info!("Hello, world! - C++ math should say 12={}", 12);
    // let obj = ffi::cfapi::APIFactory::getInstance() as *mut ffi::cfapi::APIFactory;
    // let obj =  obj as *mut ffi::cfapi::APIFactory;
    // let mut obj = unsafe { std::pin::Pin::new_unchecked(&mut *obj) };
    let (sender, recviver) = unbounded();
    let_cxx_string!(app_name = "sample");
    let_cxx_string!(app_version = "1.0");
    let_cxx_string!(log_filename = "cfapilog");
    // let_cxx_string!(usage = "External");
    // obj.as_mut().initialize(&app_name, &app_version, true, &log_filename, "External");
    let user_event_handler = MyUserEventHandler::default_rust_owned();
    let session_event_handler = MySessionEventHandler::default_rust_owned();
    let message_event_handler = MyMessageEventHandler::default_rust_owned();
    message_event_handler.as_ref().borrow_mut().debug = false;
    message_event_handler.as_ref().borrow_mut().sender = Some(sender);
    // let pin_user_event_handler: Pin<&mut cfapi::UserEventHandler> =
    //     unsafe { std::pin::Pin::new_unchecked(user_event_handler as &mut cfapi::UserEventHandler) };
    // let pin_session_event_handler: Pin<&mut cfapi::SessionEventHandler> =
    //     unsafe { std::pin::Pin::new_unchecked(session_event_handler) };
    // let user_event_handler = MyUserEventHandler::new().within_unique_ptr();
    // let session_event_handler = MySessionEventHandler::new().within_unique_ptr();
    let_cxx_string!(user_name = "SINOPACNB");
    let_cxx_string!(password = "s1nopac");

    let mut api = ffi::APIFactoryWrap::new(
        &app_name,
        &app_version,
        true,
        &log_filename,
        "External",
        &user_name,
        &password,
        // pin_user_event_handler,
        // pin_session_event_handler,
        user_event_handler.as_ref().borrow().as_ref(),
        session_event_handler.as_ref().borrow().as_ref(),
        // &user_event_handler,
        // &session_event_handler,
    )
    .within_unique_ptr();
    // let api = CppUniquePtrPin::new(api);
    api.pin_mut().setSessionConfig(
        cfapi::SessionConfig_Parameters::MAX_USER_THREADS_LONG,
        autocxx::c_long(10),
    );
    let_cxx_string!(host_info = "216.221.213.14:7022");
    api.pin_mut().setHostConfig(host_info, false, true);
    api.pin_mut()
        .registerMessageEventHandler(message_event_handler.as_ref().borrow().as_ref());
    // api.pin_mut().setConnectionConfig(
    //     cfapi::HostConfig_Parameters::CONFLATION_INTERVAL_LONG,
    //     false,
    // );
    // api.pin_mut().setConnectionConfig(
    //     cfapi::HostConfig_Parameters::CONFLATION_INTERVAL_LONG,
    //     autocxx::c_long(10),
    // );

    api.pin_mut().startSession();
    let_cxx_string!(src_id = "533");
    let_cxx_string!(symbol = "AAPL");
    let req = api
        .pin_mut()
        .sendRequest(&src_id, &symbol, cfapi::Commands::QUERYSNAPANDSUBSCRIBE);
    debug!("req: {}", req);
    // let_cxx_string!(src_id = "533");
    // let_cxx_string!(symbol = "{^A}");
    // let req = api.pin_mut().sendRequest(
    //     &src_id,
    //     &symbol,
    //     cfapi::Commands::QUERYSNAPANDSUBSCRIBEWILDCARD,
    //     // cfapi::Commands::QUERYWILDCARD,
    // );
    // debug!("req: {}", req);
    std::thread::spawn(move || {
        let solclient = SolClient::new(SolClientLogLevel::Notice);
        match solclient {
            Ok(mut solclient) => {
                let session_props = SessionProps::default()
                    .host("128.110.5.101:55555")
                    .vpn("sinopac")
                    .username("shioaji")
                    .password("shioaji111");
                let r = solclient.connect(session_props);
                info!("connect: {:?}", r);
                let event_recv = solclient.get_event_receiver();
                let _th_event = std::thread::spawn(move || loop {
                    match event_recv.recv() {
                        Ok(event) => {
                            info!("{:?}", event);
                        }
                        Err(e) => {
                            tracing::error!("recv event error: {:?}", e);
                            break;
                        }
                    }
                });
                match recviver.recv() {
                    Ok(data) => {
                        info!("recv data: {:?}", data);
                        let mut msg = SolMsg::new().unwrap();
                        msg.set_topic("api/v1/test");
                        let msgp_data = rmp_serde::to_vec_named(&data).unwrap();
                        msg.set_binary_attachment(&msgp_data);
                        // std::thread::sleep(std::time::Duration::from_secs(5));
                        let rt = solclient.send_msg(&msg);
                        info!("send msg: {:?}", rt);
                    }
                    Err(e) => {
                        tracing::error!("recv error: {:?}", e);
                    }
                }

                std::thread::sleep(std::time::Duration::from_secs(30 * 60));
            }
            Err(e) => {
                info!("create solclient error: {:?}", e);
            }
        }
    });

    std::thread::sleep(std::time::Duration::from_secs(30 * 60));
    // let mut session = api.pin_mut().getSession();
    // let mut session_config = session.getSessionConfig();//.pin_mut().getSessionConfig();

    // api
    // ffi::cfapi::APIFactory::createSession(obj.as_mut(), &user_name, &password, &user_event_handler);
    // obj.as_mut().createSession(&user_name, &password, &user_event_handler);
    // obj.as_mut();

    // ffi::cfapi::APIFactory::initialize(&app_name, &app_version, true, &log_filename);
    // let mut instance = ffi::cfapi::APIFactory::getInstance();
    // instance.as_mut().unwrap().initialize("sample", "1.0.0", true, "cfapilog");
    // ffi::print_value(123);
    // println!("Hello, world! - C++ math should say 12={}", ffi::DoMath(4));
    // let mut goat = ffi::Goat::new().within_box();
    // goat.as_mut().add_a_horn();
    // goat.as_mut().add_a_horn();
    // assert_eq!(
    //     goat.describe().as_ref().unwrap().to_string_lossy(),
    //     "This goat has 2 horns."
    // );
    // assert_eq!(ffi::do_math(12, 13), 25);
    // print!("do_math: {}\n", ffi::do_math(20, 30));
    // let mut goat = ffi::Goat::new().within_unique_ptr(); // returns a cxx::UniquePtr, i.e. a std::unique_ptr
    // goat.pin_mut().add_a_horn();
    // goat.pin_mut().add_a_horn();
    // assert_eq!(goat.describe().as_ref().unwrap().to_string_lossy(), "This goat has 2 horns.");
}
