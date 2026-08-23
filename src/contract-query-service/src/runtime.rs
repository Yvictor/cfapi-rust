use crate::{
    backend::{BackendConfig, BackendConfigError, ContractBackend, ReadinessState},
    cache::{CacheLimits, FreshnessPolicy, SourceCache},
    cfapi_adapter::{CfapiAdapter, QueryXrefEventBridge},
    http,
    query::{PendingRegistry, QueryError, RegistryLimits},
    service::{CfapiCommandBus, ContractService, ServiceConfig},
};
use cfapi::{
    api::{CFAPIConfig, ConnectionConfig, SessionConfig, CFAPI},
    binding::{SessionEvent, SessionEvent_Types},
    session_event::SessionEventHandlerExt,
};
use parking_lot::Mutex;
use salvo::conn::Listener;
use salvo::prelude::{Server, TcpListener};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    env,
    net::SocketAddr,
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use thiserror::Error;
use tokio::{sync::Notify, task::JoinSet};

const HTTP_BIND: &str = "CONTRACT_HTTP_BIND";
const USERNAME: &str = "CFAPI_USERNAME";
const USERNAME_ALIAS: &str = "CFAPI_USER";
const PASSWORD: &str = "CFAPI_PASSWORD";
const PASSWORD_ALIAS: &str = "CFAPI_PASS";
const HOSTS: &str = "CFAPI_HOSTS";
const HOSTS_ALIAS: &str = "CFAPI_HOST";
const SOURCES: &str = "CFAPI_SOURCES";

/// Runtime configuration intentionally has no `Debug` implementation because it
/// owns the CFAPI password. Configuration errors identify keys, never values.
pub struct RuntimeConfig {
    bind: SocketAddr,
    username: String,
    password: String,
    hosts: Vec<String>,
    sources: Vec<u16>,
    max_user_threads: i64,
    max_csp_threads: i64,
    max_request_queue_size: i64,
    connection_compression: bool,
    registry_limits: RegistryLimits,
    cache_limits: CacheLimits,
    service: ServiceConfig,
    backend: BackendConfig,
    auto_sync: bool,
    shutdown_timeout: Duration,
}

impl RuntimeConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_values(env::vars())
    }

    pub fn from_values<I, K, V>(values: I) -> Result<Self, ConfigError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let values: HashMap<String, String> = values
            .into_iter()
            .map(|(key, value)| (key.into(), value.into()))
            .collect();
        let bind = parse_value(&values, HTTP_BIND, "0.0.0.0:8080")?;
        let username = required_with_alias(&values, USERNAME, USERNAME_ALIAS)?;
        let password = required_with_alias(&values, PASSWORD, PASSWORD_ALIAS)?;
        let hosts = csv(&required_with_alias(&values, HOSTS, HOSTS_ALIAS)?, HOSTS)?;
        if !hosts.iter().all(|host| valid_host(host)) {
            return Err(ConfigError::Invalid(HOSTS));
        }
        let sources = parse_sources(values.get(SOURCES).map(String::as_str).unwrap_or("533,534"))?;
        let max_user_threads = parse_value(&values, "CFAPI_MAX_USER_THREADS", "0")?;
        let max_csp_threads = parse_value(&values, "CFAPI_MAX_CSP_THREADS", "32")?;
        let max_request_queue_size =
            parse_value(&values, "CFAPI_MAX_REQUEST_QUEUE_SIZE", "100000")?;
        if max_request_queue_size < 100_000 {
            return Err(ConfigError::OutOfRange("CFAPI_MAX_REQUEST_QUEUE_SIZE"));
        }
        let command_capacity = nonzero(
            parse_value(&values, "CFAPI_COMMAND_CAPACITY", "256")?,
            "CFAPI_COMMAND_CAPACITY",
        )?;
        let query_timeout_ms = nonzero(
            parse_value(&values, "CONTRACT_QUERY_TIMEOUT_MS", "5000")?,
            "CONTRACT_QUERY_TIMEOUT_MS",
        )?;
        let fresh_for = parse_value(&values, "CONTRACT_CACHE_FRESH_SECONDS", "300")?;
        let max_stale_for = parse_value(&values, "CONTRACT_CACHE_MAX_STALE_SECONDS", "86400")?;
        if max_stale_for < fresh_for {
            return Err(ConfigError::OutOfRange("CONTRACT_CACHE_MAX_STALE_SECONDS"));
        }
        let freshness = FreshnessPolicy {
            fresh_for,
            max_stale_for,
        };
        let registry_limits = RegistryLimits {
            max_exact: nonzero(
                parse_value(&values, "CONTRACT_MAX_EXACT_QUERIES", "256")?,
                "CONTRACT_MAX_EXACT_QUERIES",
            )?,
            max_exact_per_source: nonzero(
                parse_value(&values, "CONTRACT_MAX_EXACT_PER_SOURCE", "32")?,
                "CONTRACT_MAX_EXACT_PER_SOURCE",
            )?,
            max_whole_source: nonzero(
                parse_value(&values, "CONTRACT_MAX_WHOLE_SOURCE_QUERIES", "2")?,
                "CONTRACT_MAX_WHOLE_SOURCE_QUERIES",
            )?,
            item_queue_capacity: nonzero(
                parse_value(&values, "CONTRACT_ITEM_QUEUE_CAPACITY", "4096")?,
                "CONTRACT_ITEM_QUEUE_CAPACITY",
            )?,
            max_records: nonzero(
                parse_value(&values, "CONTRACT_QUERY_MAX_RECORDS", "100000")?,
                "CONTRACT_QUERY_MAX_RECORDS",
            )?,
            max_owned_bytes: nonzero(
                parse_value(&values, "CONTRACT_QUERY_MAX_OWNED_BYTES", "268435456")?,
                "CONTRACT_QUERY_MAX_OWNED_BYTES",
            )?,
            tombstone_capacity: nonzero(
                parse_value(&values, "CONTRACT_TOMBSTONE_CAPACITY", "65536")?,
                "CONTRACT_TOMBSTONE_CAPACITY",
            )?,
            tombstone_ttl: Duration::from_secs(nonzero(
                parse_value(&values, "CONTRACT_TOMBSTONE_TTL_SECONDS", "360")?,
                "CONTRACT_TOMBSTONE_TTL_SECONDS",
            )?),
        };
        if registry_limits.max_exact_per_source > registry_limits.max_exact {
            return Err(ConfigError::OutOfRange("CONTRACT_MAX_EXACT_PER_SOURCE"));
        }
        if registry_limits.max_whole_source < sources.len() {
            return Err(ConfigError::OutOfRange("CONTRACT_MAX_WHOLE_SOURCE_QUERIES"));
        }
        let cache_limits = CacheLimits {
            max_records: nonzero(
                parse_value(&values, "CONTRACT_CACHE_MAX_RECORDS", "100000")?,
                "CONTRACT_CACHE_MAX_RECORDS",
            )?,
            max_generation_bytes: nonzero(
                parse_value(&values, "CONTRACT_CACHE_MAX_BYTES", "268435456")?,
                "CONTRACT_CACHE_MAX_BYTES",
            )?,
            max_overlays: nonzero(
                parse_value(&values, "CONTRACT_CACHE_MAX_OVERLAYS", "10000")?,
                "CONTRACT_CACHE_MAX_OVERLAYS",
            )?,
        };
        let service = ServiceConfig {
            freshness,
            query_timeout: Duration::from_millis(query_timeout_ms),
            negative_ttl: parse_value(&values, "CONTRACT_NEGATIVE_TTL_SECONDS", "30")?,
            command_capacity,
        };
        let backend = BackendConfig {
            freshness,
            max_retained_jobs: nonzero(
                parse_value(&values, "CONTRACT_MAX_RETAINED_JOBS", "1024")?,
                "CONTRACT_MAX_RETAINED_JOBS",
            )?,
        };

        Ok(Self {
            bind,
            username,
            password,
            hosts,
            sources,
            max_user_threads,
            max_csp_threads,
            max_request_queue_size,
            connection_compression: parse_value(&values, "CFAPI_CONNECTION_COMPRESSION", "true")?,
            registry_limits,
            cache_limits,
            service,
            backend,
            auto_sync: parse_value(&values, "CONTRACT_AUTO_SYNC", "true")?,
            shutdown_timeout: Duration::from_millis(nonzero(
                parse_value(&values, "CONTRACT_SHUTDOWN_TIMEOUT_MS", "10000")?,
                "CONTRACT_SHUTDOWN_TIMEOUT_MS",
            )?),
        })
    }

    pub fn bind(&self) -> SocketAddr {
        self.bind
    }

    pub fn hosts(&self) -> &[String] {
        &self.hosts
    }

    pub fn sources(&self) -> &[u16] {
        &self.sources
    }

    pub fn auto_sync(&self) -> bool {
        self.auto_sync
    }

    pub fn max_user_threads(&self) -> i64 {
        self.max_user_threads
    }

    pub fn max_csp_threads(&self) -> i64 {
        self.max_csp_threads
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ConfigError {
    #[error("required environment variable {0} is missing")]
    Missing(&'static str),
    #[error("environment variable {0} is invalid")]
    Invalid(&'static str),
    #[error("environment variable {0} is outside its supported range")]
    OutOfRange(&'static str),
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error(transparent)]
    Configuration(#[from] ConfigError),
    #[error("failed to initialize CFAPI adapter: {0}")]
    Adapter(QueryError),
    #[error("failed to initialize HTTP backend: {0}")]
    Backend(#[from] BackendConfigError),
    #[error("runtime startup failed: {0}")]
    Startup(String),
    #[error("HTTP server failed: {0}")]
    Server(#[from] std::io::Error),
    #[error("shutdown signal handler failed: {0}")]
    Signal(std::io::Error),
}

pub struct RuntimeSessionEventHandler {
    readiness: Arc<ReadinessState>,
    registry: Arc<PendingRegistry>,
    session_gate: Arc<SessionGate>,
    sources: Mutex<SourceAvailability>,
}

impl RuntimeSessionEventHandler {
    pub fn new(readiness: Arc<ReadinessState>, registry: Arc<PendingRegistry>) -> Self {
        Self::with_gate(
            readiness,
            registry,
            Arc::new(SessionGate::default()),
            std::iter::empty(),
        )
    }

    fn with_gate(
        readiness: Arc<ReadinessState>,
        registry: Arc<PendingRegistry>,
        session_gate: Arc<SessionGate>,
        required_sources: impl IntoIterator<Item = u16>,
    ) -> Self {
        Self {
            readiness,
            registry,
            session_gate,
            sources: Mutex::new(SourceAvailability::new(required_sources)),
        }
    }

    fn handle_status(&self, status: RuntimeSessionStatus) {
        match status {
            RuntimeSessionStatus::AvailableAllSources => {
                self.sources.lock().mark_all_available();
                self.set_available(true);
            }
            RuntimeSessionStatus::AvailableSource(source_id) => {
                let ready = self.sources.lock().mark_available(source_id);
                self.set_available(ready);
            }
            RuntimeSessionStatus::Established => {
                self.sources.lock().clear();
                self.set_available(false);
            }
            RuntimeSessionStatus::RecoverySource(source_id) => {
                self.sources.lock().mark_unavailable(source_id);
                self.fail_pending();
            }
            RuntimeSessionStatus::RecoveryAll | RuntimeSessionStatus::Unavailable => {
                self.sources.lock().clear();
                self.fail_pending();
            }
        }
    }

    fn set_available(&self, available: bool) {
        self.readiness.set_cfapi_session(available);
        self.session_gate.set_available(available);
    }

    fn fail_pending(&self) {
        self.set_available(false);
        self.registry.fail_all(QueryError::SessionUnavailable);
    }
}

impl SessionEventHandlerExt for RuntimeSessionEventHandler {
    fn on_session_event(&mut self, event: &SessionEvent) {
        let status = match event.getType() {
            SessionEvent_Types::CFAPI_SESSION_AVAILABLE_ALLSOURCES => {
                Some(RuntimeSessionStatus::AvailableAllSources)
            }
            SessionEvent_Types::CFAPI_SESSION_AVAILABLE_SOURCES => {
                u16::try_from(event.getSourceID().0)
                    .ok()
                    .map(RuntimeSessionStatus::AvailableSource)
            }
            SessionEvent_Types::CFAPI_SESSION_ESTABLISHED => {
                Some(RuntimeSessionStatus::Established)
            }
            SessionEvent_Types::CFAPI_SESSION_RECOVERY => Some(RuntimeSessionStatus::RecoveryAll),
            SessionEvent_Types::CFAPI_SESSION_RECOVERY_SOURCES => Some(
                u16::try_from(event.getSourceID().0)
                    .map(RuntimeSessionStatus::RecoverySource)
                    .unwrap_or(RuntimeSessionStatus::RecoveryAll),
            ),
            SessionEvent_Types::CFAPI_SESSION_UNAVAILABLE => {
                Some(RuntimeSessionStatus::Unavailable)
            }
            _ => None,
        };
        if let Some(status) = status {
            self.handle_status(status);
        }
    }
}

#[derive(Clone, Copy)]
enum RuntimeSessionStatus {
    Established,
    AvailableAllSources,
    AvailableSource(u16),
    RecoveryAll,
    RecoverySource(u16),
    Unavailable,
}

struct SourceAvailability {
    required: BTreeSet<u16>,
    available: BTreeSet<u16>,
}

impl SourceAvailability {
    fn new(required: impl IntoIterator<Item = u16>) -> Self {
        Self {
            required: required.into_iter().collect(),
            available: BTreeSet::new(),
        }
    }

    fn mark_all_available(&mut self) {
        self.available.clone_from(&self.required);
    }

    fn mark_available(&mut self, source_id: u16) -> bool {
        if self.required.is_empty() || self.required.contains(&source_id) {
            self.available.insert(source_id);
        }
        self.required.is_empty() || self.required.is_subset(&self.available)
    }

    fn mark_unavailable(&mut self, source_id: u16) {
        self.available.remove(&source_id);
    }

    fn clear(&mut self) {
        self.available.clear();
    }
}

#[derive(Default)]
struct SessionGate {
    available: AtomicBool,
    changed: Notify,
}

impl SessionGate {
    fn set_available(&self, available: bool) {
        self.available.store(available, Ordering::Release);
        if available {
            self.changed.notify_waiters();
        }
    }

    async fn wait_available(&self) {
        loop {
            let notified = self.changed.notified();
            if self.available.load(Ordering::Acquire) {
                return;
            }
            notified.await;
        }
    }
}

pub async fn run(config: RuntimeConfig) -> Result<(), RuntimeError> {
    let RuntimeParts {
        backend,
        registry,
        readiness,
        mut startup_tasks,
        bind,
        shutdown_timeout,
    } = compose(config)?;

    let acceptor = TcpListener::new(bind)
        .try_bind()
        .await
        .map_err(|error| RuntimeError::Startup(error.to_string()))?;
    let server = Server::new(acceptor);
    let handle = server.handle();
    let serve = server.try_serve(http::service(backend));
    tokio::pin!(serve);

    let result = tokio::select! {
        result = &mut serve => result.map_err(RuntimeError::Server),
        signal = tokio::signal::ctrl_c() => {
            signal.map_err(RuntimeError::Signal)?;
            readiness.set_cfapi_session(false);
            readiness.set_query_coordinator(false);
            readiness.set_sync_coordinator(false);
            registry.shutdown();
            startup_tasks.abort_all();
            while startup_tasks.join_next().await.is_some() {}
            handle.stop_graceful(Some(shutdown_timeout));
            serve.await.map_err(RuntimeError::Server)
        }
    };

    registry.shutdown();
    startup_tasks.abort_all();
    while startup_tasks.join_next().await.is_some() {}
    result
}

struct RuntimeParts {
    backend: Arc<ContractBackend>,
    registry: Arc<PendingRegistry>,
    readiness: Arc<ReadinessState>,
    startup_tasks: JoinSet<()>,
    bind: SocketAddr,
    shutdown_timeout: Duration,
}

fn compose(config: RuntimeConfig) -> Result<RuntimeParts, RuntimeError> {
    let registry = Arc::new(PendingRegistry::new(config.registry_limits.clone()));
    let readiness = Arc::new(ReadinessState::not_ready());
    let session_gate = Arc::new(SessionGate::default());
    let event_bridge = QueryXrefEventBridge::new(Arc::clone(&registry));
    let session_handler = RuntimeSessionEventHandler::with_gate(
        Arc::clone(&readiness),
        Arc::clone(&registry),
        Arc::clone(&session_gate),
        config.sources.iter().copied(),
    );
    let cfapi_config = CFAPIConfig::new(
        "contract-query-service".to_owned(),
        env!("CARGO_PKG_VERSION").to_owned(),
        false,
        "contract-query-service-cfapi.log".to_owned(),
        "External".to_owned(),
        config.username,
        config.password,
        60,
    );
    let session_config = SessionConfig::default()
        .with_multi_threaded_api_connections(true)
        .with_max_user_threads(config.max_user_threads)
        .with_max_csp_threads(config.max_csp_threads)
        .with_max_request_queue_size(config.max_request_queue_size);
    let connection_config =
        ConnectionConfig::default().with_compression(config.connection_compression);
    let hosts = config.hosts;
    let adapter = CfapiAdapter::spawn(move || {
        let mut api = CFAPI::new(
            cfapi_config,
            Vec::new(),
            vec![Box::new(session_handler)],
            vec![Box::new(event_bridge)],
            Vec::new(),
        );
        api.set_session_config(&session_config);
        for host in &hosts {
            api.set_connection_config(host, &connection_config);
        }
        api.start();
        api
    })
    .map_err(RuntimeError::Adapter)?;
    let command_bus = CfapiCommandBus::start(
        adapter,
        Arc::clone(&registry),
        config.service.command_capacity,
    );

    let mut caches = BTreeMap::new();
    let mut services = BTreeMap::new();
    for source in config.sources.iter().copied() {
        let cache = Arc::new(SourceCache::new(source, config.cache_limits));
        let service = Arc::new(ContractService::with_command_bus(
            Arc::clone(&cache),
            command_bus.clone(),
            config.service.clone(),
        ));
        caches.insert(source, cache);
        services.insert(source, service);
    }
    readiness.set_query_coordinator(true);
    readiness.set_sync_coordinator(true);

    let readiness_caches: Vec<_> = caches.values().cloned().collect();
    let backend = Arc::new(ContractBackend::new(
        services.clone(),
        caches,
        config.backend,
        Arc::clone(&readiness),
    )?);
    let mut startup_tasks = JoinSet::new();
    let cache_readiness = Arc::clone(&readiness);
    startup_tasks.spawn(async move {
        loop {
            if readiness_caches
                .iter()
                .all(|cache| cache.snapshot().is_complete())
            {
                cache_readiness.set_cache(true);
                return;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    });
    if config.auto_sync {
        for (source, service) in services {
            let session_gate = Arc::clone(&session_gate);
            startup_tasks.spawn(async move {
                session_gate.wait_available().await;
                // A failed startup sync leaves this source incomplete and readiness
                // false. A later successful manual sync is observed by the monitor.
                let _ = service.sync_source(source).await;
            });
        }
    }

    Ok(RuntimeParts {
        backend,
        registry,
        readiness,
        startup_tasks,
        bind: config.bind,
        shutdown_timeout: config.shutdown_timeout,
    })
}

fn csv(value: &str, key: &'static str) -> Result<Vec<String>, ConfigError> {
    let parsed: Vec<_> = value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect();
    if parsed.is_empty() {
        Err(ConfigError::Invalid(key))
    } else {
        Ok(parsed)
    }
}

fn required_with_alias(
    values: &HashMap<String, String>,
    primary: &'static str,
    alias: &'static str,
) -> Result<String, ConfigError> {
    let selected = values
        .get(primary)
        .or_else(|| values.get(alias))
        .filter(|value| !value.trim().is_empty());
    selected.cloned().ok_or(ConfigError::Missing(primary))
}

fn parse_sources(value: &str) -> Result<Vec<u16>, ConfigError> {
    let mut sources = csv(value, SOURCES)?
        .into_iter()
        .map(|source| u16::from_str(&source).map_err(|_| ConfigError::Invalid(SOURCES)))
        .collect::<Result<Vec<_>, _>>()?;
    sources.sort_unstable();
    sources.dedup();
    if sources.contains(&0) {
        return Err(ConfigError::OutOfRange(SOURCES));
    }
    Ok(sources)
}

fn valid_host(value: &str) -> bool {
    let Some((host, port)) = value.rsplit_once(':') else {
        return false;
    };
    let host = host.trim_matches(['[', ']']);
    !host.is_empty() && !host.chars().any(char::is_whitespace) && port.parse::<u16>().is_ok()
}

fn parse_value<T>(
    values: &HashMap<String, String>,
    key: &'static str,
    default: &str,
) -> Result<T, ConfigError>
where
    T: FromStr,
{
    values
        .get(key)
        .map(String::as_str)
        .unwrap_or(default)
        .parse()
        .map_err(|_| ConfigError::Invalid(key))
}

fn nonzero<T>(value: T, key: &'static str) -> Result<T, ConfigError>
where
    T: Default + PartialEq,
{
    if value == T::default() {
        Err(ConfigError::OutOfRange(key))
    } else {
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handler(
        required_sources: impl IntoIterator<Item = u16>,
    ) -> (
        RuntimeSessionEventHandler,
        Arc<PendingRegistry>,
        Arc<SessionGate>,
    ) {
        let readiness = Arc::new(ReadinessState::not_ready());
        let registry = Arc::new(PendingRegistry::new(RegistryLimits::default()));
        let gate = Arc::new(SessionGate::default());
        let handler = RuntimeSessionEventHandler::with_gate(
            Arc::clone(&readiness),
            Arc::clone(&registry),
            Arc::clone(&gate),
            required_sources,
        );
        (handler, registry, gate)
    }

    #[test]
    fn established_is_not_ready_until_every_configured_source_is_available() {
        let (handler, _, gate) = handler([533, 534]);

        handler.handle_status(RuntimeSessionStatus::Established);
        assert!(!gate.available.load(Ordering::Acquire));

        handler.handle_status(RuntimeSessionStatus::AvailableSource(533));
        assert!(!gate.available.load(Ordering::Acquire));

        handler.handle_status(RuntimeSessionStatus::AvailableSource(534));
        assert!(gate.available.load(Ordering::Acquire));
    }

    #[test]
    fn all_sources_event_marks_the_configured_set_ready() {
        let (handler, _, gate) = handler([533, 534]);
        handler.handle_status(RuntimeSessionStatus::AvailableAllSources);
        assert!(gate.available.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn recovery_and_unavailable_fail_pending_and_clear_readiness() {
        for status in [
            RuntimeSessionStatus::RecoverySource(533),
            RuntimeSessionStatus::RecoveryAll,
            RuntimeSessionStatus::Unavailable,
        ] {
            let (handler, registry, gate) = handler([533, 534]);
            handler.handle_status(RuntimeSessionStatus::AvailableAllSources);
            let query = registry.register_exact(1, 533).expect("register query");

            handler.handle_status(status);

            assert!(!gate.available.load(Ordering::Acquire));
            assert_eq!(query.receive().await, Err(QueryError::SessionUnavailable));
            assert_eq!(registry.pending_count(), 0);
        }
    }
}
