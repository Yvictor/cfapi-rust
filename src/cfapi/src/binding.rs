use std::fmt::{Debug, Display};

use super::message_event::{DefaultMessageEventHandler, MessageEventHandlerExt};
use super::session_event::{DefaultSessionEventHandler, SessionEventHandlerExt};
use super::stat_event::{DefaultStatisticsEventHandler, StatisticsEventHandlerExt};
use super::user_event::{DefaultUserEventHandler, UserEventHandlerExt};

use autocxx::prelude::*;
use autocxx::subclass::*;

include_cpp! {
    #include "cfapi.h"
    #include "api.h"
    // #include "APIFactory.h"
    safety!(unsafe_ffi)
    generate!("cfapi::Session")
    // generate!("cfapi::APIFactory")
    generate!("APIFactoryWrap")
    generate!("GetEventReader")
    generate!("GetDate")
    generate!("GetTime")
    generate!("cfapi::SessionConfig")
    generate!("cfapi::HostConfig")
    generate!("cfapi::UserInfo")
    subclass!("cfapi::UserEventHandler", BaseUserEventHandler)
    generate!("cfapi::UserEvent")
    subclass!("cfapi::SessionEventHandler", BaseSessionEventHandler)
    generate!("cfapi::SessionEvent")
    subclass!("cfapi::MessageEventHandler", BaseMessageEventHandler)
    generate!("cfapi::MessageEvent")
    generate!("cfapi::MessageReader")
    generate!("cfapi::StatisticsEvent")
    subclass!("cfapi::StatisticsEventHandler", BaseStatisticsEventHandler)
    generate!("cfapi::ValueTypes")
    generate!("cfapi::Commands")
    generate!("cfapi::DateTime")
    generate!("cfapi::Date")
    generate!("cfapi::Time")
}

pub use cfapi::*;
pub use ffi::*;

#[subclass]
#[derive(Default)]
pub struct BaseUserEventHandler {
    user_event_handlers: std::sync::Mutex<Vec<Box<dyn UserEventHandlerExt + Send + 'static>>>,
    with_default: bool,
}

impl BaseUserEventHandler {
    pub fn new(user_event_handlers: Vec<Box<dyn UserEventHandlerExt + Send + 'static>>) -> Self {
        let mut me = Self::default();
        let user_event_handlers = if user_event_handlers.is_empty() {
            me.with_default = true;
            vec![Box::new(DefaultUserEventHandler) as Box<dyn UserEventHandlerExt + Send>]
        } else {
            me.with_default = false;
            user_event_handlers
        };
        me.with_user_event_hanlder(user_event_handlers)
    }
    pub fn with_user_event_hanlder(
        mut self,
        user_event_handlers: Vec<Box<dyn UserEventHandlerExt + Send + 'static>>,
    ) -> Self {
        self.user_event_handlers = std::sync::Mutex::new(user_event_handlers);
        self.with_default = false;
        self
    }
    pub fn add_user_event_handler(
        &mut self,
        user_event_handler: Box<dyn UserEventHandlerExt + Send + 'static>,
    ) {
        let mut handlers = self.user_event_handlers.lock().unwrap();
        if self.with_default {
            handlers.pop();
            self.with_default = false;
        };
        handlers.push(user_event_handler);
    }

    pub fn clear_user_event_handlers(&mut self) {
        self.user_event_handlers.lock().unwrap().clear();
        self.with_default = false;
    }
}

impl cfapi::UserEventHandler_methods for BaseUserEventHandler {
    fn onUserEvent(&mut self, event: &cfapi::UserEvent) {
        let mut handlers = self.user_event_handlers.lock().unwrap();
        for handler in handlers.iter_mut() {
            handler.on_user_event(event);
        }
    }
}

#[subclass]
#[derive(Default)]
pub struct BaseSessionEventHandler {
    handlers: std::sync::Mutex<Vec<Box<dyn SessionEventHandlerExt + Send + 'static>>>,
    with_default: bool,
}

impl BaseSessionEventHandler {
    pub fn new(handlers: Vec<Box<dyn SessionEventHandlerExt + Send + 'static>>) -> Self {
        let mut me = Self::default();
        let handlers = if handlers.is_empty() {
            me.with_default = true;
            vec![Box::new(DefaultSessionEventHandler) as Box<dyn SessionEventHandlerExt + Send>]
        } else {
            me.with_default = false;
            handlers
        };
        me.with_hanlder(handlers)
    }
    pub fn with_hanlder(
        mut self,
        handlers: Vec<Box<dyn SessionEventHandlerExt + Send + 'static>>,
    ) -> Self {
        self.handlers = std::sync::Mutex::new(handlers);
        self.with_default = false;
        self
    }

    pub fn add_handler(&mut self, handler: Box<dyn SessionEventHandlerExt + Send + 'static>) {
        let mut handlers = self.handlers.lock().unwrap();
        if self.with_default {
            handlers.pop();
            self.with_default = false;
        };
        handlers.push(handler);
    }

    pub fn clear_handlers(&mut self) {
        self.handlers.lock().unwrap().clear();
        self.with_default = false;
    }
}

impl cfapi::SessionEventHandler_methods for BaseSessionEventHandler {
    fn onSessionEvent(&mut self, event: &cfapi::SessionEvent) {
        let mut handlers = self.handlers.lock().unwrap();
        for handler in handlers.iter_mut() {
            handler.on_session_event(event);
        }
    }
}

#[subclass]
#[derive(Default)]
pub struct BaseMessageEventHandler {
    handlers: std::sync::Mutex<Vec<Box<dyn MessageEventHandlerExt + Send + Sync + 'static>>>,
    with_default: bool,
}

impl BaseMessageEventHandler {
    pub fn new(handlers: Vec<Box<dyn MessageEventHandlerExt + Send + Sync + 'static>>) -> Self {
        let mut me = Self::default();
        let handlers = if handlers.is_empty() {
            me.with_default = true;
            vec![Box::new(DefaultMessageEventHandler::default())
                as Box<dyn MessageEventHandlerExt + Send + Sync>]
        } else {
            me.with_default = false;
            handlers
        };
        me.with_hanlder(handlers)
    }
    pub fn with_hanlder(
        mut self,
        handlers: Vec<Box<dyn MessageEventHandlerExt + Send + Sync + 'static>>,
    ) -> Self {
        self.handlers = std::sync::Mutex::new(handlers);
        self.with_default = false;
        self
    }

    pub fn add_handler(
        &mut self,
        handler: Box<dyn MessageEventHandlerExt + Send + Sync + 'static>,
    ) {
        let mut handlers = self.handlers.lock().unwrap();
        if self.with_default {
            handlers.pop();
            self.with_default = false;
        };
        handlers.push(handler);
    }

    pub fn clear_handlers(&mut self) {
        self.handlers.lock().unwrap().clear();
        self.with_default = false;
    }
}

impl cfapi::MessageEventHandler_methods for BaseMessageEventHandler {
    fn onMessageEvent(&mut self, event: &cfapi::MessageEvent) {
        let mut handlers = self.handlers.lock().unwrap();
        for handler in handlers.iter_mut() {
            handler.on_message_event(event);
        }
    }
}

#[subclass]
#[derive(Default)]
pub struct BaseStatisticsEventHandler {
    handlers: std::sync::Mutex<Vec<Box<dyn StatisticsEventHandlerExt + Send + 'static>>>,
    with_default: bool,
}

impl BaseStatisticsEventHandler {
    pub fn new(handlers: Vec<Box<dyn StatisticsEventHandlerExt + Send + 'static>>) -> Self {
        let mut me = Self::default();
        let handlers = if handlers.is_empty() {
            me.with_default = true;
            vec![Box::new(DefaultStatisticsEventHandler)
                as Box<dyn StatisticsEventHandlerExt + Send>]
        } else {
            me.with_default = false;
            handlers
        };
        me.with_hanlder(handlers)
    }
    pub fn with_hanlder(
        mut self,
        handlers: Vec<Box<dyn StatisticsEventHandlerExt + Send + 'static>>,
    ) -> Self {
        self.handlers = std::sync::Mutex::new(handlers);
        self.with_default = false;
        self
    }

    pub fn add_handler(&mut self, handler: Box<dyn StatisticsEventHandlerExt + Send + 'static>) {
        let mut handlers = self.handlers.lock().unwrap();
        if self.with_default {
            handlers.pop();
            self.with_default = false;
        };
        handlers.push(handler);
    }

    pub fn clear_handlers(&mut self) {
        self.handlers.lock().unwrap().clear();
        self.with_default = false;
    }
}

impl cfapi::StatisticsEventHandler_methods for BaseStatisticsEventHandler {
    fn onStatisticsEvent(&mut self, event: &cfapi::StatisticsEvent) {
        let mut handlers = self.handlers.lock().unwrap();
        for handler in handlers.iter_mut() {
            handler.on_statistics_event(event);
        }
    }
}

impl Debug for MessageEvent_Types {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageEvent_Types::IMAGE_PART => write!(f, "IMAGE_PART"),
            MessageEvent_Types::IMAGE_COMPLETE => write!(f, "IMAGE_COMPLETE"),
            MessageEvent_Types::REFRESH => write!(f, "REFRESH"),
            MessageEvent_Types::STATUS => write!(f, "STATUS"),
            MessageEvent_Types::UPDATE => write!(f, "UPDATE"),
        }
    }
}

impl Display for MessageEvent_Types {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageEvent_Types::IMAGE_PART => write!(f, "IMAGE_PART"),
            MessageEvent_Types::IMAGE_COMPLETE => write!(f, "IMAGE_COMPLETE"),
            MessageEvent_Types::REFRESH => write!(f, "REFRESH"),
            MessageEvent_Types::STATUS => write!(f, "STATUS"),
            MessageEvent_Types::UPDATE => write!(f, "UPDATE"),
        }
    }
}
