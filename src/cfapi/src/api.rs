use std::cell::RefCell;
use std::rc::Rc;

use super::binding::{
    APIFactoryWrap, BaseMessageEventHandler, BaseSessionEventHandler, BaseStatisticsEventHandler,
    BaseUserEventHandler, Commands,
};
use super::message_event::MessageEventHandlerExt;
use super::session_event::SessionEventHandlerExt;
use super::stat_event::StatisticsEventHandlerExt;
use super::user_event::UserEventHandlerExt;
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
    // interval in seconds to report statistics
    statistics_interval: i32,
}

impl Default for CFAPIConfig {
    fn default() -> Self {
        CFAPIConfig::new(
            "app".to_owned(),
            "1.0".to_owned(),
            false,
            "cfapilog".to_owned(),
            "External".to_owned(),
            "username".to_owned(),
            "password".to_owned(),
            60,
        )
    }
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
        statistics_interval: i32,
    ) -> Self {
        CFAPIConfig {
            app_name,
            app_version,
            debug,
            log_filename,
            usage,
            username,
            password,
            statistics_interval,
        }
    }

    pub fn with_app_name(mut self, app_name: &str) -> Self {
        self.app_name = app_name.to_owned();
        self
    }

    pub fn with_app_version(mut self, app_version: &str) -> Self {
        self.app_version = app_version.to_owned();
        self
    }

    pub fn with_debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    pub fn with_log_filename(mut self, log_filename: &str) -> Self {
        self.log_filename = log_filename.to_owned();
        self
    }

    /// Parameter "usage" is used for only internal purpose;
    pub fn with_usage(mut self, usage: &String) -> Self {
        self.usage = usage.to_owned();
        self
    }

    pub fn with_username(mut self, username: &str) -> Self {
        self.username = username.to_owned();
        self
    }

    pub fn with_password(mut self, password: &str) -> Self {
        self.password = password.to_owned();
        self
    }

    pub fn with_statistics_interval(mut self, statistics_interval: i32) -> Self {
        self.statistics_interval = statistics_interval;
        self
    }
}

/// SessionConfig
/// multithreaded_api_connections: Indicate whether API should create mulitple threads to handle multiple CSP connections.  Default is false.
/// max_user_threads: When multithreaded_api_connections=true, this indicates the maximum number of user-side threads the API should create.  If set to 0 (default), <0, or not set, the API will open as many threads as it can use.
/// max_csp_threads: When multithreaded_api_connections=true, this indicates the maximum number of CSP-side (backend) threads the API should create.  If set to 0 (default), <0, or not set, the API will open as many threads as it can use.
/// max_request_queue_size: Maximum number of requests to be queued before sending to the CSP.  Default is 100000 (without watchlist) or 10000000 (with watchlist); valid range is 100000-50000000 (requests). When the queue is full, any further send() requests will fail.
/// watchlist: Indicate whether API should use watchlist to manage requests.  Default is false.
/// max_watchlist_size: Maximum number of requests to be added to watchlist.  Default is 10000000 (requests).
/// queue_depth_threshold_percent: Threshold to trigger CFAPI_SESSION_RECEIVE_QUEUE_ABOVE_THRESHOLD and CFAPI_SESSION_RECEIVE_QUEUE_BELOW_THRESHOLD SessionEvents.  Default is 70%; valid range is 1-101.
pub struct SessionConfig {
    /// Indicate whether API should create mulitple threads to handle multiple CSP connections.  Default is false.
    pub multithreaded_api_connections: bool,
    /// When multithreaded_api_connections=true, this indicates the maximum number of user-side threads the API should create.  If set to 0 (default), <0, or not set, the API will open as many threads as it can use.
    pub max_user_threads: i64,
    /// When multithreaded_api_connections=true, this indicates the maximum number of CSP-side (backend) threads the API should create.  If set to 0 (default), <0, or not set, the API will open as many threads as it can use.
    pub max_csp_threads: i64,
    /// Maximum number of requests to be queued before sending to the CSP.  Default is 100000 (without watchlist) or 10000000 (with watchlist); valid range is 100000-50000000 (requests)
    /// When the queue is full, any further send() requests will fail.
    pub max_request_queue_size: i64,
    /// Indicate whether API should use watchlist to manage requests.  Default is false.
    pub watchlist: bool,
    /// Maximum number of requests to be added to watchlist.  Default is 10000000 (requests)
    pub max_watchlist_size: i64,
    /// Threshold to trigger CFAPI_SESSION_RECEIVE_QUEUE_ABOVE_THRESHOLD and CFAPI_SESSION_RECEIVE_QUEUE_BELOW_THRESHOLD SessionEvents.  Default is 70%; valid range is 1-101.
    pub queue_depth_threshold_percent: i64,
}

impl SessionConfig {
    /// Indicate whether API should create mulitple threads to handle multiple CSP connections.  Default is false.
    pub fn with_multi_threaded_api_connections(
        mut self,
        multithreaded_api_connections: bool,
    ) -> Self {
        self.multithreaded_api_connections = multithreaded_api_connections;
        self
    }
    /// When multithreaded_api_connections=true, this indicates the maximum number of user-side threads the API should create.  If set to 0 (default), <0, or not set, the API will open as many threads as it can use.
    pub fn with_max_user_threads(mut self, max_user_threads: i64) -> Self {
        self.max_user_threads = max_user_threads;
        self
    }
    /// When multithreaded_api_connections=true, this indicates the maximum number of CSP-side (backend) threads the API should create.  If set to 0 (default), <0, or not set, the API will open as many threads as it can use.
    pub fn with_max_csp_threads(mut self, max_csp_threads: i64) -> Self {
        self.max_csp_threads = max_csp_threads;
        self
    }
    /// Maximum number of requests to be queued before sending to the CSP.  Default is 100000 (without watchlist) or 10000000 (with watchlist); valid range is 100000-50000000 (requests)
    /// When the queue is full, any further send() requests will fail.
    pub fn with_max_request_queue_size(mut self, max_request_queue_size: i64) -> Self {
        self.max_request_queue_size = max_request_queue_size;
        self
    }
    /// Indicate whether API should use watchlist to manage requests.  Default is false.
    pub fn with_watchlist(mut self, watchlist: bool) -> Self {
        self.watchlist = watchlist;
        self
    }
    /// Maximum number of requests to be added to watchlist.  Default is 10000000 (requests)
    pub fn with_max_watchlist_size(mut self, max_watchlist_size: i64) -> Self {
        self.max_watchlist_size = max_watchlist_size;
        self
    }
    /// Threshold to trigger CFAPI_SESSION_RECEIVE_QUEUE_ABOVE_THRESHOLD and CFAPI_SESSION_RECEIVE_QUEUE_BELOW_THRESHOLD SessionEvents.  Default is 70%; valid range is 1-101.
    pub fn with_queue_depth_threshold_percent(
        mut self,
        queue_depth_threshold_percent: i64,
    ) -> Self {
        self.queue_depth_threshold_percent = queue_depth_threshold_percent;
        self
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        SessionConfig {
            multithreaded_api_connections: false,
            max_user_threads: 0,
            max_csp_threads: 0,
            max_request_queue_size: 100000,
            watchlist: false,
            max_watchlist_size: 10000000,
            queue_depth_threshold_percent: 70,
        }
    }
}

pub struct ConnectionConfig {
    /// Is this a backup CSP?  Default is false.
    pub backup: bool,
    /// Use compression between CSP and API. Default is true
    pub compression: bool,
    /// Indicate which messages are conflatable. Default is false
    pub conflation_indicator: bool,
    /// Maximum time to hold conflatable data (in milliseconds).  Default is 1000 (ms)
    pub conflation_interval: i64,
    /// Maximum time to wait for a heartbeat message from the CSP.  Default is 5 (seconds)
    /// Note: during initialization, this is the maximum time to wait for any response from the CSP.
    pub read_timeout: i64,
    /// Maximum time to wait to establish a TCP connection to the CSP.  Default is 5 (seconds)
    pub connection_timeout: i64,
    /// Maximum number of consecutive reconnects to the CSP without successfully initializing.  Default is 5
    pub connection_retry_limit: i64,
    /// Size in megabytes of internal buffer for incoming messages from the CSP.  Default is 1 (megabyte); valid range is 1-256 (MB)
    /// Note: there is one queue per backend CSP, so each one will be allocated with this size
    pub queue_size: i64,
    /// Maximum time socket select should block, waiting for a connection to be ready; Default is 200 millisec
    pub blocking_connection_time_limit: i64,
    /// Type of conflation.  1: trade-safe (Default); 2: intervalized; 3: Just-In-Time (JIT)
    pub conflation_type: i64,
    /// Threshold to trigger JIT conflation.  This is the percent of the buffer used before feed starts conflating. Default is 25%; valid range is 1-75.  Only valid when CONFLATION_TYPE_LONG is set to 3 (JIT).
    pub jit_conflation_threshold_percent: i64,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        ConnectionConfig {
            backup: false,
            compression: true,
            conflation_indicator: false,
            conflation_interval: 1000,
            read_timeout: 5,
            connection_timeout: 5,
            connection_retry_limit: 5,
            queue_size: 1,
            blocking_connection_time_limit: 200,
            conflation_type: 1,
            jit_conflation_threshold_percent: 25,
        }
    }
}

impl ConnectionConfig {
    /// Is this a backup CSP?  Default is false.
    pub fn with_backup(mut self, backup: bool) -> Self {
        self.backup = backup;
        self
    }
    /// Use compression between CSP and API.  Default is true
    pub fn with_compression(mut self, compression: bool) -> Self {
        self.compression = compression;
        self
    }
    /// Indicate which messages are conflatable.  Default is false
    pub fn with_conflation_indicator(mut self, conflation_indicator: bool) -> Self {
        self.conflation_indicator = conflation_indicator;
        self
    }
    /// Maximum time to hold conflatable data (in milliseconds).  Default is 1000 (ms)
    /// Note: during initialization, this is the maximum time to wait for any response from the CSP.
    pub fn with_conflation_interval(mut self, conflation_interval: i64) -> Self {
        self.conflation_interval = conflation_interval;
        self
    }
    /// Maximum time to wait for a heartbeat message from the CSP.  Default is 5 (seconds)
    pub fn with_read_timeout(mut self, read_timeout: i64) -> Self {
        self.read_timeout = read_timeout;
        self
    }
    /// Maximum time to wait to establish a TCP connection to the CSP.  Default is 5 (seconds)
    pub fn with_connection_timeout(mut self, connection_timeout: i64) -> Self {
        self.connection_timeout = connection_timeout;
        self
    }
    /// Maximum number of consecutive reconnects to the CSP without successfully initializing.  Default is 5
    pub fn with_connection_retry_limit(mut self, connection_retry_limit: i64) -> Self {
        self.connection_retry_limit = connection_retry_limit;
        self
    }
    /// Size in megabytes of internal buffer for incoming messages from the CSP.  Default is 1 (megabyte); valid range is 1-256 (MB)
    pub fn with_queue_size(mut self, queue_size: i64) -> Self {
        self.queue_size = queue_size;
        self
    }
    /// Maximum time socket select should block, waiting for a connection to be ready; Default is 200 millisec
    pub fn with_blocking_connection_time_limit(
        mut self,
        blocking_connection_time_limit: i64,
    ) -> Self {
        self.blocking_connection_time_limit = blocking_connection_time_limit;
        self
    }
    /// Type of conflation.  1: trade-safe (Default); 2: intervalized; 3: Just-In-Time (JIT)
    pub fn with_conflation_type(mut self, conflation_type: i64) -> Self {
        self.conflation_type = conflation_type;
        self
    }
    /// Threshold to trigger JIT conflation.  This is the percent of the buffer used before feed starts conflating. Default is 25%; valid range is 1-75.  Only valid when CONFLATION_TYPE_LONG is set to 3 (JIT).
    pub fn with_jit_conflation_threshold_percent(
        mut self,
        jit_conflation_threshold_percent: i64,
    ) -> Self {
        self.jit_conflation_threshold_percent = jit_conflation_threshold_percent;
        self
    }
}

pub struct CFAPI {
    api: UniquePtr<APIFactoryWrap>,
    _user_event_handler: Rc<RefCell<BaseUserEventHandler>>,
    _session_event_handler: Rc<RefCell<BaseSessionEventHandler>>,
    _message_event_handler: Rc<RefCell<BaseMessageEventHandler>>,
    _statistics_event_handler: Rc<RefCell<BaseStatisticsEventHandler>>,
}

impl CFAPI {
    pub fn new(
        config: CFAPIConfig,
        user_event_handlers: Vec<Box<dyn UserEventHandlerExt + 'static>>,
        session_event_handlers: Vec<Box<dyn SessionEventHandlerExt>>,
        message_event_handlers: Vec<Box<dyn MessageEventHandlerExt>>,
        statistics_event_handlers: Vec<Box<dyn StatisticsEventHandlerExt>>,
    ) -> Self {
        let user_event_handler =
            BaseUserEventHandler::new_rust_owned(BaseUserEventHandler::new(user_event_handlers));
        let session_event_handler = BaseSessionEventHandler::new_rust_owned(
            BaseSessionEventHandler::new(session_event_handlers),
        );
        let message_event_handler = BaseMessageEventHandler::new_rust_owned(
            BaseMessageEventHandler::new(message_event_handlers),
        );
        let statistics_event_handler = BaseStatisticsEventHandler::new_rust_owned(
            BaseStatisticsEventHandler::new(statistics_event_handlers),
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
        api.pin_mut().registerStatisticsEventHandler(
            statistics_event_handler.as_ref().borrow().as_ref(),
            autocxx::c_int(config.statistics_interval),
        );
        CFAPI {
            api,
            _user_event_handler: user_event_handler,
            _session_event_handler: session_event_handler,
            _message_event_handler: message_event_handler,
            _statistics_event_handler: statistics_event_handler,
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

    pub fn add_session_event_handler(
        &mut self,
        session_event_handler: Box<dyn SessionEventHandlerExt>,
    ) {
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

    pub fn add_message_event_handler(
        &mut self,
        message_event_handler: Box<dyn MessageEventHandlerExt>,
    ) {
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

    pub fn add_statistics_event_handler(
        &mut self,
        statistics_event_handler: Box<dyn StatisticsEventHandlerExt>,
    ) {
        self._statistics_event_handler
            .as_ref()
            .borrow_mut()
            .add_handler(statistics_event_handler);
    }

    pub fn clear_statistics_event_handlers(&mut self) {
        self._statistics_event_handler
            .as_ref()
            .borrow_mut()
            .clear_handlers();
    }

    pub fn set_session_config(&mut self, session_config: &SessionConfig) {
        self.api.pin_mut().setSessionConfigBool(
            crate::binding::SessionConfig_Parameters::MULTITHREADED_API_CONNECTIONS_BOOL,
            session_config.multithreaded_api_connections,
        );
        self.api.pin_mut().setSessionConfigInt(
            crate::binding::SessionConfig_Parameters::MAX_USER_THREADS_LONG,
            autocxx::c_long(session_config.max_user_threads),
        );
        self.api.pin_mut().setSessionConfigInt(
            crate::binding::SessionConfig_Parameters::MAX_CSP_THREADS_LONG,
            autocxx::c_long(session_config.max_csp_threads),
        );
        self.api.pin_mut().setSessionConfigInt(
            crate::binding::SessionConfig_Parameters::MAX_REQUEST_QUEUE_SIZE_LONG,
            autocxx::c_long(session_config.max_request_queue_size),
        );
        self.api.pin_mut().setSessionConfigBool(
            crate::binding::SessionConfig_Parameters::WATCHLIST_BOOL,
            session_config.watchlist,
        );
        self.api.pin_mut().setSessionConfigInt(
            crate::binding::SessionConfig_Parameters::MAX_WATCHLIST_SIZE_LONG,
            autocxx::c_long(session_config.max_watchlist_size),
        );
        self.api.pin_mut().setSessionConfigInt(
            crate::binding::SessionConfig_Parameters::QUEUE_DEPTH_THRESHOLD_PERCENT_LONG,
            autocxx::c_long(session_config.queue_depth_threshold_percent),
        );
    }

    pub fn set_connection_config(&mut self, host_info: &str, connection_config: &ConnectionConfig) {
        let_cxx_string!(host_info = host_info);
        self.api.pin_mut().setConnectionConfig(
            host_info,
            connection_config.backup,
            connection_config.compression,
            connection_config.conflation_indicator,
            autocxx::c_long(connection_config.conflation_interval),
            autocxx::c_long(connection_config.read_timeout),
            autocxx::c_long(connection_config.connection_timeout),
            autocxx::c_long(connection_config.connection_retry_limit),
            autocxx::c_long(connection_config.queue_size),
            autocxx::c_long(connection_config.blocking_connection_time_limit),
            autocxx::c_long(connection_config.conflation_type),
            autocxx::c_long(connection_config.jit_conflation_threshold_percent),
        );
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
