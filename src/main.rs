// Use all the autocxx types which might be handy.
use autocxx::prelude::*;
// use autocxx::subclass::prelude::*;
use autocxx::subclass::*;
use cxx::let_cxx_string;
// use cxx::{}

// include_cpp! {
//     #include "sample.hpp"
//     safety!(unsafe_ffi)
//     generate!("print_value") // allowlist a function
//     generate!("DoMath")
//     generate!("Goat")
// }
pub mod session {
    autocxx::include_cpp! {
        #include "cfapi.h"
        name!(ffisession)
        safety!(unsafe_ffi)
        generate!("cfapi::Session")
    }
    pub use ffisession::*;
}

pub mod user {
    autocxx::include_cpp! {
        #include "cfapi.h"
        name!(ffiuser)
        safety!(unsafe_ffi)
        generate!("cfapi::UserInfo")
    }
    pub use ffiuser::*;
}

include_cpp! {
    #include "cfapi.h"
    #include "api.h"
    // #include "APIFactory.h"
    safety!(unsafe_ffi)
    // generate!("cfapi::Session")
    // generate!("cfapi::APIFactory")
    generate!("APIFactoryWrap")
    extern_cpp_type!("cfapi::Session", crate::session::cfapi::Session)
    // generate!("cfapi::UserEventHandler")
    // generate!("cfapi::UserInfo")
    extern_cpp_type!("cfapi::UserInfo", crate::user::cfapi::UserInfo)
    subclass!("cfapi::UserEventHandler", MyUserEventHandler)
    generate!("cfapi::UserEvent")
    subclass!("cfapi::SessionEventHandler", MySessionEventHandler)
    generate!("cfapi::SessionEvent")
    subclass!("cfapi::MessageEventHandler", MyMessageEventHandler)
    generate!("cfapi::MessageEvent")
}
use ffi::*;

#[subclass]//(superclass("cfapi::UserEventHandler"))
#[derive(Default)]
pub struct MyUserEventHandler;

impl cfapi::UserEventHandler_methods for MyUserEventHandler {
    fn onUserEvent(&mut self, event: &cfapi::UserEvent) {
        let _event_type = event.getType();
        println!("on event: {:?}", event.getRetCode());
    }
}

#[subclass]//(superclass("cfapi::SessionEventHandler"))
#[derive(Default)]
pub struct MySessionEventHandler;

impl cfapi::SessionEventHandler_methods for MySessionEventHandler {
    fn onSessionEvent(&mut self, event: &cfapi::SessionEvent) {
        let event_type = event.getType() as cfapi::SessionEvent_Types; // as cfapi::SessionEvent::Types
        match event_type {
            cfapi::SessionEvent_Types::CFAPI_SESSION_UNAVAILABLE => {
                println!("session unavailable");
            }
            cfapi::SessionEvent_Types::CFAPI_SESSION_ESTABLISHED => {
                println!("session established");
            }
            cfapi::SessionEvent_Types::CFAPI_SESSION_RECOVERY => {
                println!("session recovery");
            }
            cfapi::SessionEvent_Types::CFAPI_SESSION_RECOVERY_SOURCES => {
                println!("session recovery sourceID = {:?}", event.getSourceID());
            }
            cfapi::SessionEvent_Types::CFAPI_CDD_LOADED => {
                println!("cdd loaded version: {}", event.getCddVersion());
            }
            cfapi::SessionEvent_Types::CFAPI_SESSION_AVAILABLE_ALLSOURCES => {
                let source_id = event.getSourceID();
                println!("session available all sources sourceID = {:?}", source_id);
            }
            cfapi::SessionEvent_Types::CFAPI_SESSION_AVAILABLE_SOURCES => {
                let source_id = event.getSourceID();
                println!("session available sources sourceID = {:?}", source_id);
            }
            cfapi::SessionEvent_Types::CFAPI_SESSION_RECEIVE_QUEUE_ABOVE_THRESHOLD => {
                println!(
                    "session receive queue above threshold {:?}",
                    event.getQueueDepth()
                );
            }
            cfapi::SessionEvent_Types::CFAPI_SESSION_RECEIVE_QUEUE_BELOW_THRESHOLD => {
                println!(
                    "session receive queue below threshold {:?}",
                    event.getQueueDepth()
                );
            }
            cfapi::SessionEvent_Types::CFAPI_SESSION_JIT_START_CONFLATING => {
                println!("session jit start conflation");
            }
            cfapi::SessionEvent_Types::CFAPI_SESSION_JIT_STOP_CONFLATING => {
                println!("session jit stop conflation");
            }
            cfapi::SessionEvent_Types::CFAPI_SESSION_SOURCE_ADDED => {
                println!("session source added sourceID = {:?}", event.getSourceID());
            }
            cfapi::SessionEvent_Types::CFAPI_SESSION_SOURCE_REMOVED => {
                println!(
                    "session source removed sourceID = {:?}",
                    event.getSourceID()
                );
            } // _ => {
              //     println!("Ubknown event type");
              // }
        }
    }
}

#[subclass]
#[derive(Default)]
pub struct MyMessageEventHandler;

impl cfapi::MessageEventHandler_methods for MyMessageEventHandler {
    fn onMessageEvent(&mut self, event: &cfapi::MessageEvent) {
        println!("onMessageEvent");
        let _event_type = event.getType() as cfapi::MessageEvent_Types;
    }
}

// impl Drop for MyUserEventHandler {
//     fn drop(&mut self) {

//     }
// }

fn main() {
    println!("Hello, world! - C++ math should say 12={}", 12);
    // let obj = ffi::cfapi::APIFactory::getInstance() as *mut ffi::cfapi::APIFactory;
    // let obj =  obj as *mut ffi::cfapi::APIFactory;
    // let mut obj = unsafe { std::pin::Pin::new_unchecked(&mut *obj) };
    let_cxx_string!(app_name = "sample");
    let_cxx_string!(app_version = "1.0");
    let_cxx_string!(log_filename = "cfapilog");
    // let_cxx_string!(usage = "External");
    // obj.as_mut().initialize(&app_name, &app_version, true, &log_filename, "External");
    let user_event_handler = MyUserEventHandler::default_rust_owned();
    let session_event_handler = MySessionEventHandler::default_rust_owned();
    // let pin_user_event_handler: Pin<&mut cfapi::UserEventHandler> =
    //     unsafe { std::pin::Pin::new_unchecked(user_event_handler as &mut cfapi::UserEventHandler) };
    // let pin_session_event_handler: Pin<&mut cfapi::SessionEventHandler> =
    //     unsafe { std::pin::Pin::new_unchecked(session_event_handler) };
    // let user_event_handler = MyUserEventHandler::new().within_unique_ptr();
    // let session_event_handler = MySessionEventHandler::new().within_unique_ptr();
    let_cxx_string!(user_name = "SINOPACNB");
    let_cxx_string!(password = "s1nopac");

    let _api = ffi::APIFactoryWrap::new(
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
