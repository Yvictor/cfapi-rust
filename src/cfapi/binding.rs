
use ahash::RandomState;
use autocxx::prelude::*;
use autocxx::subclass::*;
use crossbeam_channel::Sender;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, span, Level};
use std::fs::OpenOptions;
use std::io::prelude::Write;
use crate::cfapi::event_reader::{EventReader, EventReaderSerConfig};
use crate::cfapi::user_event::UserEventHandlerExt;

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
    subclass!("cfapi::UserEventHandler", BaseUserEventHandler)
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

pub use ffi::*;
pub use cfapi::*;


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


#[subclass] 
#[derive(Default)]
pub struct BaseUserEventHandler {
    user_event_handler: Option<Box<dyn UserEventHandlerExt + 'static>>
}

impl BaseUserEventHandler {
    pub fn new(user_event_handler: Box<dyn UserEventHandlerExt + 'static>) -> Self {
        let me = Self::default();
        me.with_user_event_hanlder(user_event_handler)
    }
    pub fn with_user_event_hanlder(mut self, user_event_handler: Box<dyn UserEventHandlerExt + 'static>) -> Self {
        self.user_event_handler = Some(user_event_handler);
        self
    }
}

impl cfapi::UserEventHandler_methods for BaseUserEventHandler {
    fn onUserEvent(&mut self, event: &cfapi::UserEvent) {
        match self.user_event_handler {
            Some(ref mut handler) => {
                handler.on_user_event(event);
            }
            None => {
                match event.getType() {
                    cfapi::UserEvent_Types::AUTHORIZATION_FAILURE => {
                        info!("AUTHORIZATION_FAILURE");
                    }
                    cfapi::UserEvent_Types::AUTHORIZATION_SUCCESS => {
                        info!("AUTHORIZATION_SUCCESS");
                    }
                }
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
        let event_reader = EventReader::new(
            &event,
            EventReaderSerConfig::default()
                .with_event_type(true)
                .with_src(true),
        );
        // debug!("JSON: {}", event_reader.to_json().unwrap());
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open("event.json")
            .unwrap();
        if let Err(e) = writeln!(file, "{}", event_reader.to_json().unwrap()) {
            error!("write event error: {:?}", e);
        }
        let span = span!(Level::INFO, "onMessageEvent");
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
                        while reader.as_mut().next() != autocxx::c_int(-1) {
                            debug!(
                                "{}({})",
                                reader.as_mut().getTokenName(),
                                i32::from(reader.as_mut().getTokenNumber())
                            )
                        }
                        // if reader.as_mut().find(autocxx::c_int(8)) {
                        //     debug!("update price: {}", reader.as_mut().getValueAsDouble());
                        //     (*data).price = reader.as_mut().getValueAsDouble()
                        // }
                        // if reader.as_mut().find(autocxx::c_int(9)) {
                        //     debug!("update volume: {}", reader.as_mut().getValueAsInteger());
                        //     (*data).volume = reader.as_mut().getValueAsInteger()
                        // }
                        if reader.as_mut().find(autocxx::c_int(10)) {
                            debug!("update ask price: {}", reader.as_mut().getValueAsDouble());
                            (*data).ask_price = reader.as_mut().getValueAsDouble()
                        }
                        if reader.as_mut().find(autocxx::c_int(11)) {
                            debug!("update ask size: {}", reader.as_mut().getValueAsInteger());
                            (*data).ask_size = reader.as_mut().getValueAsInteger()
                        }
                        if reader.as_mut().find(autocxx::c_int(12)) {
                            debug!("update bid price: {}", reader.as_mut().getValueAsDouble());
                            (*data).bid_price = reader.as_mut().getValueAsDouble()
                        }
                        if reader.as_mut().find(autocxx::c_int(13)) {
                            debug!("update bid size: {}", reader.as_mut().getValueAsInteger());
                            (*data).bid_size = reader.as_mut().getValueAsInteger()
                        }
                        if reader.as_mut().find(autocxx::c_int(14)) {
                            debug!("update price: {}", reader.as_mut().getValueAsDouble());
                            (*data).price = reader.as_mut().getValueAsDouble()
                        }
                        if reader.as_mut().find(autocxx::c_int(16)) {
                            (*data).ts = reader.as_mut().getValueAsDouble()
                        }
                        // if reader.as_mut().find(autocxx::c_int(8)) {
                        //     debug!("update price: {}", reader.as_mut().getValueAsDouble());
                        //     (*data).price = reader.as_mut().getValueAsDouble()
                        // }
                        if reader.as_mut().find(autocxx::c_int(22)) {
                            debug!("update volume: {}", reader.as_mut().getValueAsInteger());
                            (*data).volume = reader.as_mut().getValueAsInteger()
                        }
                        debug!("update data: {:?}", *data);
                    }
                    // let data = self.maps.get(symbol.to_str().unwrap()).unwrap();
                    // info!("upd data: {:?}", *data);
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
