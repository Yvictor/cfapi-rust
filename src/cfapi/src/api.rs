use std::cell::RefCell;
use std::rc::Rc;

use super::message_event::MessageEventHandlerExt;
use super::session_event::SessionEventHandlerExt;
use super::user_event::UserEventHandlerExt;
use crate::binding::{
    APIFactoryWrap, BaseMessageEventHandler, BaseSessionEventHandler, BaseUserEventHandler,
    Commands,
};
use autocxx::subclass::CppSubclass;
use autocxx::WithinUniquePtr;
use cxx::{let_cxx_string, UniquePtr};

pub struct CFAPIConfig {
    app_name: String,
    app_version: String,
    debug: bool,
    log_filename: String,
    usage: String,
    username: String,
    password: String,
}

impl CFAPIConfig {
    pub fn new(
        app_name: String,
        app_version: String,
        debug: bool,
        log_filename: String,
        usage: String,
        username: String,
        password: String,
    ) -> Self {
        CFAPIConfig {
            app_name,
            app_version,
            debug,
            log_filename,
            usage,
            username,
            password,
        }
    }
}

pub struct CFAPI {
    api: UniquePtr<APIFactoryWrap>,
    _user_event_handler: Rc<RefCell<BaseUserEventHandler>>,
    _session_event_handler: Rc<RefCell<BaseSessionEventHandler>>,
    _message_event_handler: Rc<RefCell<BaseMessageEventHandler>>,
}

impl CFAPI {
    pub fn new(
        config: CFAPIConfig,
        user_event_handlers: Vec<Box<dyn UserEventHandlerExt + 'static>>,
        session_event_handlers: Vec<Box<dyn SessionEventHandlerExt>>,
        message_event_handlers: Vec<Box<dyn MessageEventHandlerExt>>,
    ) -> Self {
        let user_event_handler =
            BaseUserEventHandler::new_rust_owned(BaseUserEventHandler::new(user_event_handlers));
        let session_event_handler = BaseSessionEventHandler::new_rust_owned(
            BaseSessionEventHandler::new(session_event_handlers),
        );
        let message_event_handler = BaseMessageEventHandler::new_rust_owned(
            BaseMessageEventHandler::new(message_event_handlers),
        );
        let_cxx_string!(app_name = config.app_name);
        let_cxx_string!(app_version = config.app_version);
        let_cxx_string!(log_filename = config.log_filename);
        let_cxx_string!(username = config.username);
        let_cxx_string!(password = config.password);
        let mut api = APIFactoryWrap::new(
            &app_name,
            &app_version,
            config.debug,
            &log_filename,
            config.usage,
            &username,
            &password,
            user_event_handler.as_ref().borrow().as_ref(),
            session_event_handler.as_ref().borrow().as_ref(),
        )
        .within_unique_ptr();
        api.pin_mut()
            .registerMessageEventHandler(message_event_handler.as_ref().borrow().as_ref());
        CFAPI {
            api,
            _user_event_handler: user_event_handler,
            _session_event_handler: session_event_handler,
            _message_event_handler: message_event_handler,
        }
    }

    pub fn add_user_event_handler(&mut self, user_event_handler: Box<dyn UserEventHandlerExt>) {
        self._user_event_handler
            .as_ref()
            .borrow_mut()
            .add_user_event_handler(user_event_handler);
    }

    pub fn clear_user_event_handlers(&mut self) {
        self._user_event_handler
            .as_ref()
            .borrow_mut()
            .clear_user_event_handlers();
    }

    pub fn add_session_event_handler(&mut self, session_event_handler: Box<dyn SessionEventHandlerExt>) {
        self._session_event_handler
            .as_ref()
            .borrow_mut()
            .add_handler(session_event_handler);
    }

    pub fn clear_session_event_handlers(&mut self) {
        self._session_event_handler
            .as_ref()
            .borrow_mut()
            .clear_handlers();
    }

    pub fn add_message_event_handler(&mut self, message_event_handler: Box<dyn MessageEventHandlerExt>) {
        self._message_event_handler
            .as_ref()
            .borrow_mut()
            .add_handler(message_event_handler);
    }

    pub fn clear_message_event_handlers(&mut self) {
        self._message_event_handler
            .as_ref()
            .borrow_mut()
            .clear_handlers();
    }


    pub fn set_session_config(&mut self, max_user_threads: i64) {
        self.api.pin_mut().setSessionConfig(
            crate::binding::SessionConfig_Parameters::MAX_USER_THREADS_LONG,
            autocxx::c_long(max_user_threads),
        );
    }

    pub fn set_host_config(&mut self, host_info: &str, backup: bool, compression: bool) {
        let_cxx_string!(host_info = host_info);
        self.api
            .pin_mut()
            .setHostConfig(host_info, backup, compression);
    }

    pub fn start(&mut self) {
        self.api.pin_mut().startSession();
    }

    pub fn request(&mut self, src_id: &str, symbol: &str, command: Commands) {
        let_cxx_string!(src_id = src_id);
        let_cxx_string!(symbol = symbol);
        self.api.pin_mut().sendRequest(&src_id, &symbol, command);
    }
}
