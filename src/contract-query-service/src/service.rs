use contract_query_service::{
    cache::{
        decide_freshness, Consistency, FreshnessDecision, FreshnessPolicy, RefreshFailure,
        SourceCache,
    },
    convert_query_xref_row,
    query::{
        CallbackClassification, PendingRegistry, QueryError, QueryTag, RequestId, SendDisposition,
    },
    ContractKey, ContractView, OwnedQueryXrefRow,
};
use parking_lot::Mutex;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use thiserror::Error;
use time::OffsetDateTime;
use tokio::sync::{mpsc, oneshot, Notify};

#[derive(Clone, Debug)]
pub struct ServiceConfig {
    pub freshness: FreshnessPolicy,
    pub query_timeout: Duration,
    pub negative_ttl: u64,
    pub command_capacity: usize,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            freshness: FreshnessPolicy {
                fresh_for: 300,
                max_stale_for: 86_400,
            },
            query_timeout: Duration::from_secs(5),
            negative_ttl: 30,
            command_capacity: 256,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ContractLookup {
    pub contract: Arc<ContractView>,
    pub freshness: FreshnessDecision,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ServiceError {
    #[error(transparent)]
    Query(#[from] QueryError),
    #[error("unsupported source {0}")]
    UnsupportedSource(u16),
    #[error("invalid QueryXref row: {0}")]
    InvalidRow(String),
    #[error("cache update failed: {0}")]
    Cache(String),
    #[error("whole-source staging failed: {0}")]
    Staging(String),
}

#[derive(Clone, Debug)]
pub enum OwnerQuery {
    Exact { source_id: u16, symbol: String },
    WholeSource { source_id: u16 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnerSendError {
    QueueFull,
}

/// Owns all mutable CFAPI request state. Implementations must not retain request
/// borrows after `send_prepared` returns.
pub trait CfapiCommandOwner: Send + 'static {
    fn prepare(&mut self, query: &OwnerQuery) -> Result<QueryTag, QueryError>;
    fn send_prepared(&mut self, query: &OwnerQuery, tag: QueryTag) -> Result<(), OwnerSendError>;
}

struct Command {
    request_id: RequestId,
    query: OwnerQuery,
    reply: oneshot::Sender<Result<SendDisposition, QueryError>>,
}

#[derive(Clone)]
struct CommandHandle {
    sender: mpsc::Sender<Command>,
    registry: Arc<PendingRegistry>,
}

impl CommandHandle {
    fn start<O>(mut owner: O, registry: Arc<PendingRegistry>, capacity: usize) -> Self
    where
        O: CfapiCommandOwner,
    {
        assert!(capacity > 0);
        let (sender, mut receiver) = mpsc::channel::<Command>(capacity);
        let worker_registry = Arc::clone(&registry);
        std::thread::Builder::new()
            .name("cfapi-contract-owner".to_owned())
            .spawn(move || {
                while let Some(command) = receiver.blocking_recv() {
                    let result = process_command(&mut owner, &worker_registry, &command);
                    let _ = command.reply.send(result);
                }
            })
            .expect("failed to start CFAPI contract command owner");
        Self { sender, registry }
    }

    async fn submit(
        &self,
        request_id: RequestId,
        query: OwnerQuery,
    ) -> Result<SendDisposition, QueryError> {
        let (reply, receiver) = oneshot::channel();
        if let Err(error) = self.sender.try_send(Command {
            request_id,
            query,
            reply,
        }) {
            let error = match error {
                mpsc::error::TrySendError::Full(_) => QueryError::LocalBackpressure,
                mpsc::error::TrySendError::Closed(_) => QueryError::SessionUnavailable,
            };
            self.registry.cancel(request_id);
            return Err(error);
        }
        match receiver.await {
            Ok(result) => result,
            Err(_) => {
                self.registry.cancel(request_id);
                Err(QueryError::SessionUnavailable)
            }
        }
    }
}

fn process_command<O: CfapiCommandOwner>(
    owner: &mut O,
    registry: &PendingRegistry,
    command: &Command,
) -> Result<SendDisposition, QueryError> {
    let tag = match owner.prepare(&command.query) {
        Ok(tag) => tag,
        Err(error) => {
            registry.cancel(command.request_id);
            return Err(error);
        }
    };
    if let Err(error) = registry.bind(command.request_id, tag) {
        registry.cancel(command.request_id);
        return Err(error);
    }
    match owner.send_prepared(&command.query, tag) {
        Ok(()) => registry.mark_sent(command.request_id, tag),
        Err(OwnerSendError::QueueFull) => {
            registry.rollback_send_queue_full(command.request_id, tag);
            Err(QueryError::SendQueueFull)
        }
    }
}

struct ExactFlight {
    result: Mutex<Option<Result<(), ServiceError>>>,
    notify: Notify,
}

impl ExactFlight {
    fn new() -> Self {
        Self {
            result: Mutex::new(None),
            notify: Notify::new(),
        }
    }

    fn finish(&self, result: Result<(), ServiceError>) {
        *self.result.lock() = Some(result);
        self.notify.notify_waiters();
    }

    async fn wait(&self) -> Result<(), ServiceError> {
        loop {
            let notified = self.notify.notified();
            if let Some(result) = self.result.lock().clone() {
                return result;
            }
            notified.await;
        }
    }
}

struct ExactLeaderGuard<'a> {
    cache: &'a SourceCache,
    flights: &'a Mutex<HashMap<ContractKey, Arc<ExactFlight>>>,
    key: ContractKey,
    flight: Arc<ExactFlight>,
    armed: bool,
}

impl ExactLeaderGuard<'_> {
    fn complete(mut self, result: Result<(), ServiceError>) {
        self.cleanup(result);
        self.armed = false;
    }

    fn cleanup(&self, result: Result<(), ServiceError>) {
        self.cache.finish_exact(&self.key);
        self.flight.finish(result);
        self.flights.lock().remove(&self.key);
    }
}

impl Drop for ExactLeaderGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.cleanup(Err(ServiceError::Query(QueryError::Cancelled)));
        }
    }
}

struct PendingRequestGuard<'a> {
    registry: &'a PendingRegistry,
    request_id: RequestId,
    armed: bool,
}

impl<'a> PendingRequestGuard<'a> {
    fn new(registry: &'a PendingRegistry, request_id: RequestId) -> Self {
        Self {
            registry,
            request_id,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for PendingRequestGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.registry.cancel(self.request_id);
        }
    }
}

pub struct ContractService {
    cache: Arc<SourceCache>,
    registry: Arc<PendingRegistry>,
    commands: CommandHandle,
    config: ServiceConfig,
    next_request_id: AtomicU64,
    flights: Mutex<HashMap<ContractKey, Arc<ExactFlight>>>,
}

impl ContractService {
    pub fn new<O>(
        cache: Arc<SourceCache>,
        registry: Arc<PendingRegistry>,
        owner: O,
        config: ServiceConfig,
    ) -> Self
    where
        O: CfapiCommandOwner,
    {
        let commands = CommandHandle::start(owner, Arc::clone(&registry), config.command_capacity);
        Self {
            cache,
            registry,
            commands,
            config,
            next_request_id: AtomicU64::new(1),
            flights: Mutex::new(HashMap::new()),
        }
    }

    pub fn registry(&self) -> &Arc<PendingRegistry> {
        &self.registry
    }

    pub async fn lookup_exact(
        &self,
        source_id: u16,
        symbol: impl Into<String>,
        consistency: Consistency,
        now: OffsetDateTime,
    ) -> Result<ContractLookup, ServiceError> {
        self.ensure_source(source_id)?;
        let symbol = symbol.into();
        let now_epoch = nonnegative_epoch(now);
        let cached = self.cache.get(&symbol);
        if let Some(contract) = &cached {
            let age = row_age(contract, now);
            if age <= self.config.freshness.fresh_for {
                return Ok(ContractLookup {
                    contract: Arc::clone(contract),
                    freshness: FreshnessDecision::Fresh,
                });
            }
        } else if self.cache.is_negative(&symbol, now_epoch) {
            return Err(ServiceError::Query(QueryError::NotFound));
        }

        let key = ContractKey { source_id, symbol };
        let (flight, leader) = self.acquire_flight(&key);
        let refresh = if leader {
            let leader_guard = ExactLeaderGuard {
                cache: &self.cache,
                flights: &self.flights,
                key: key.clone(),
                flight: Arc::clone(&flight),
                armed: true,
            };
            let result = self.refresh_exact(&key, now_epoch).await;
            leader_guard.complete(result.clone());
            result
        } else {
            match tokio::time::timeout(self.config.query_timeout, flight.wait()).await {
                Ok(result) => result,
                Err(_) => Err(ServiceError::Query(QueryError::Timeout)),
            }
        };

        match refresh {
            Ok(()) => self
                .cache
                .get(&key.symbol)
                .map(|contract| ContractLookup {
                    contract,
                    freshness: FreshnessDecision::Fresh,
                })
                .ok_or(ServiceError::Query(QueryError::NotFound)),
            Err(error) => self.resolve_refresh_failure(cached, consistency, now, error),
        }
    }

    pub async fn sync_source(
        &self,
        source_id: u16,
    ) -> Result<Arc<contract_query_service::cache::Generation>, ServiceError> {
        self.ensure_source(source_id)?;
        let request_id = self.next_id();
        let query = self.registry.register_whole_source(request_id, source_id)?;
        let mut pending_guard = PendingRequestGuard::new(&self.registry, request_id);
        let command = OwnerQuery::WholeSource { source_id };
        let operation = async {
            let (sent, rows) =
                tokio::join!(self.commands.submit(request_id, command), query.drain());
            sent?;
            rows
        };
        let rows = match tokio::time::timeout(self.config.query_timeout, operation).await {
            Ok(result) => result,
            Err(_) => {
                self.registry.timeout(request_id);
                pending_guard.disarm();
                return Err(ServiceError::Query(QueryError::Timeout));
            }
        };
        pending_guard.disarm();
        let rows = rows?;

        let mut staging = self.cache.begin_sync();
        for row in rows {
            let parsed = convert_query_xref_row(row, None)
                .map_err(|error| ServiceError::InvalidRow(error.to_string()))?;
            staging
                .push(Arc::new(parsed.contract))
                .map_err(|error| ServiceError::Staging(error.to_string()))?;
        }
        self.cache
            .commit_sync(staging)
            .map_err(|error| ServiceError::Cache(error.to_string()))
    }

    fn ensure_source(&self, source_id: u16) -> Result<(), ServiceError> {
        let expected = self.cache.snapshot().source_id();
        if source_id == expected {
            Ok(())
        } else {
            Err(ServiceError::UnsupportedSource(source_id))
        }
    }

    fn acquire_flight(&self, key: &ContractKey) -> (Arc<ExactFlight>, bool) {
        let mut flights = self.flights.lock();
        if let Some(flight) = flights.get(key) {
            return (Arc::clone(flight), false);
        }
        let flight = Arc::new(ExactFlight::new());
        let leader = self.cache.try_begin_exact(key);
        if leader {
            flights.insert(key.clone(), Arc::clone(&flight));
        } else {
            flight.finish(Err(ServiceError::Query(QueryError::LocalBackpressure)));
        }
        (flight, leader)
    }

    async fn refresh_exact(&self, key: &ContractKey, now_epoch: u64) -> Result<(), ServiceError> {
        let request_id = self.next_id();
        let query = self.registry.register_exact(request_id, key.source_id)?;
        let mut pending_guard = PendingRequestGuard::new(&self.registry, request_id);
        let command = OwnerQuery::Exact {
            source_id: key.source_id,
            symbol: key.symbol.clone(),
        };
        let operation = async {
            self.commands.submit(request_id, command).await?;
            query.receive().await
        };
        let row = match tokio::time::timeout(self.config.query_timeout, operation).await {
            Ok(result) => result,
            Err(_) => {
                self.registry.timeout(request_id);
                pending_guard.disarm();
                return Err(ServiceError::Query(QueryError::Timeout));
            }
        };
        pending_guard.disarm();
        match row {
            Ok(row) => {
                let parsed = convert_query_xref_row(row, Some(key))
                    .map_err(|error| ServiceError::InvalidRow(error.to_string()))?;
                self.cache
                    .exact_upsert(Arc::new(parsed.contract))
                    .map_err(|error| ServiceError::Cache(error.to_string()))?;
                Ok(())
            }
            Err(QueryError::NotFound) => {
                self.cache
                    .exact_delete(
                        &key.symbol,
                        now_epoch.saturating_add(self.config.negative_ttl),
                    )
                    .map_err(|error| ServiceError::Cache(error.to_string()))?;
                Err(ServiceError::Query(QueryError::NotFound))
            }
            Err(error) => Err(ServiceError::Query(error)),
        }
    }

    fn resolve_refresh_failure(
        &self,
        cached: Option<Arc<ContractView>>,
        consistency: Consistency,
        now: OffsetDateTime,
        error: ServiceError,
    ) -> Result<ContractLookup, ServiceError> {
        let Some(contract) = cached else {
            return Err(error);
        };
        let failure = refresh_failure(&error);
        match decide_freshness(
            row_age(&contract, now),
            consistency,
            Some(failure),
            self.config.freshness,
        ) {
            decision @ FreshnessDecision::Stale(_) => Ok(ContractLookup {
                contract,
                freshness: decision,
            }),
            _ => Err(error),
        }
    }

    fn next_id(&self) -> RequestId {
        self.next_request_id.fetch_add(1, Ordering::Relaxed)
    }
}

fn row_age(contract: &ContractView, now: OffsetDateTime) -> u64 {
    now.unix_timestamp()
        .saturating_sub(contract.metadata.metadata_observed_at.unix_timestamp())
        .max(0) as u64
}

fn nonnegative_epoch(now: OffsetDateTime) -> u64 {
    now.unix_timestamp().max(0) as u64
}

fn refresh_failure(error: &ServiceError) -> RefreshFailure {
    match error {
        ServiceError::Query(QueryError::SessionUnavailable) => RefreshFailure::SessionUnavailable,
        ServiceError::Query(QueryError::Timeout) => RefreshFailure::Timeout,
        ServiceError::Query(QueryError::LocalBackpressure)
        | ServiceError::Query(QueryError::SendQueueFull)
        | ServiceError::Query(QueryError::ResponseBackpressure) => RefreshFailure::Backpressure,
        ServiceError::Query(QueryError::PermissionDenied) => RefreshFailure::PermissionDenied,
        ServiceError::Query(QueryError::NotFound) => RefreshFailure::NotFound,
        ServiceError::Query(QueryError::ProtocolViolation(_)) | ServiceError::InvalidRow(_) => {
            RefreshFailure::ProtocolViolation
        }
        ServiceError::UnsupportedSource(_) => RefreshFailure::UnsupportedSource,
        ServiceError::Cache(_) | ServiceError::Staging(_) => RefreshFailure::ValidationFailed,
        ServiceError::Query(_) => RefreshFailure::RefreshFailed,
    }
}

pub fn classify_late_callback(
    registry: &PendingRegistry,
    tag: QueryTag,
    row: Option<OwnedQueryXrefRow>,
) -> CallbackClassification {
    registry.on_image_complete(tag, row)
}
