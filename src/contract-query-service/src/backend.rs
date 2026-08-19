use crate::{
    cache::{
        Consistency as CacheConsistency, FreshnessDecision, FreshnessPolicy, RefreshFailure,
        SourceCache,
    },
    domain::{ContractDto as DomainContract, ContractView, CONTRACT_SCHEMA_VERSION},
    http::{
        ApiErrorCode, BackendError, ComponentHealth, Consistency, Contract, ContractHttpBackend,
        ContractResponse, CreateSyncJobRequest, CreatedSyncJob, ErrorContext,
        ExchangeContractResponse, ExchangeLookup, Freshness, FreshnessState, HealthResponse,
        JobError, LookupResult, SourceLookup, SyncJob, SyncJobStatus, TickSizeRule,
    },
    query::QueryError,
    service::{ContractLookup, ContractService, ServiceError, SyncOutcome},
};
use parking_lot::Mutex;
use serde::Serialize;
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use thiserror::Error;
use time::{format_description::well_known::Rfc3339, Duration, OffsetDateTime, UtcOffset};
use ulid::Ulid;

#[derive(Clone, Copy, Debug)]
pub struct BackendConfig {
    pub freshness: FreshnessPolicy,
    pub max_retained_jobs: usize,
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            freshness: FreshnessPolicy {
                fresh_for: 300,
                max_stale_for: 86_400,
            },
            max_retained_jobs: 1_024,
        }
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum BackendConfigError {
    #[error("service and cache source registries differ")]
    RegistryMismatch,
    #[error("max_retained_jobs must be greater than zero")]
    ZeroJobCapacity,
}

pub struct ReadinessState {
    cfapi_session: AtomicBool,
    cache: AtomicBool,
    query_coordinator: AtomicBool,
    sync_coordinator: AtomicBool,
}

impl ReadinessState {
    pub fn ready() -> Self {
        Self {
            cfapi_session: AtomicBool::new(true),
            cache: AtomicBool::new(true),
            query_coordinator: AtomicBool::new(true),
            sync_coordinator: AtomicBool::new(true),
        }
    }

    pub fn not_ready() -> Self {
        Self {
            cfapi_session: AtomicBool::new(false),
            cache: AtomicBool::new(false),
            query_coordinator: AtomicBool::new(false),
            sync_coordinator: AtomicBool::new(false),
        }
    }

    pub fn set_cfapi_session(&self, ready: bool) {
        self.cfapi_session.store(ready, Ordering::Release);
    }

    pub fn set_cache(&self, ready: bool) {
        self.cache.store(ready, Ordering::Release);
    }

    pub fn set_query_coordinator(&self, ready: bool) {
        self.query_coordinator.store(ready, Ordering::Release);
    }

    pub fn set_sync_coordinator(&self, ready: bool) {
        self.sync_coordinator.store(ready, Ordering::Release);
    }
}

struct JobState {
    jobs: HashMap<String, SyncJob>,
    order: VecDeque<String>,
    active_by_source: HashMap<u16, String>,
    idempotency: HashMap<String, (u16, String)>,
}

struct JobCoordinator {
    max_retained: usize,
    state: Mutex<JobState>,
}

impl JobCoordinator {
    fn new(max_retained: usize) -> Self {
        Self {
            max_retained,
            state: Mutex::new(JobState {
                jobs: HashMap::new(),
                order: VecDeque::new(),
                active_by_source: HashMap::new(),
                idempotency: HashMap::new(),
            }),
        }
    }

    fn create(
        &self,
        source_id: u16,
        idempotency_key: Option<String>,
        now: OffsetDateTime,
    ) -> Result<CreatedSyncJob, BackendError> {
        let mut state = self.state.lock();
        if let Some(key) = idempotency_key.as_deref() {
            if let Some((original_source, job_id)) = state.idempotency.get(key) {
                if *original_source != source_id {
                    let mut error = BackendError::new(
                        ApiErrorCode::IdempotencyConflict,
                        "Idempotency-Key was already used with a different request",
                    );
                    error.context.source_id = Some(source_id);
                    return Err(error);
                }
                if let Some(job) = state.jobs.get(job_id).cloned() {
                    return Ok(CreatedSyncJob {
                        job,
                        replayed: true,
                    });
                }
            }
        }
        if let Some(active_job_id) = state.active_by_source.get(&source_id).cloned() {
            let mut error = BackendError::new(
                ApiErrorCode::SyncAlreadyRunning,
                format!("source {source_id} already has an active synchronization job"),
            );
            error.context.active_job_id = Some(active_job_id);
            error.context.source_id = Some(source_id);
            return Err(error);
        }

        while state.jobs.len() >= self.max_retained {
            let terminal = state.order.iter().position(|job_id| {
                state.jobs.get(job_id).is_some_and(|job| {
                    matches!(
                        job.status,
                        SyncJobStatus::Succeeded | SyncJobStatus::Failed | SyncJobStatus::Cancelled
                    )
                })
            });
            let Some(position) = terminal else {
                let mut error = BackendError::new(
                    ApiErrorCode::Backpressure,
                    "synchronization job retention capacity is full",
                );
                error.retryable = true;
                return Err(error);
            };
            let evicted_id = state.order.remove(position).expect("position is valid");
            state.jobs.remove(&evicted_id);
            state
                .idempotency
                .retain(|_, (_, job_id)| job_id != &evicted_id);
        }

        let job_id = Ulid::new().to_string();
        let job = SyncJob {
            job_id: job_id.clone(),
            source_id,
            status: SyncJobStatus::Queued,
            submitted_at: format_timestamp(now),
            started_at: None,
            completed_at: None,
            received_records: 0,
            accepted_records: 0,
            rejected_records: 0,
            generation_id: None,
            error: None,
        };
        state.active_by_source.insert(source_id, job_id.clone());
        if let Some(key) = idempotency_key {
            state.idempotency.insert(key, (source_id, job_id.clone()));
        }
        state.order.push_back(job_id.clone());
        state.jobs.insert(job_id, job.clone());
        Ok(CreatedSyncJob {
            job,
            replayed: false,
        })
    }

    fn mark_running(&self, job_id: &str, now: OffsetDateTime) {
        if let Some(job) = self.state.lock().jobs.get_mut(job_id) {
            job.status = SyncJobStatus::Running;
            job.started_at = Some(format_timestamp(now));
        }
    }

    fn finish_success(&self, job_id: &str, outcome: SyncOutcome, now: OffsetDateTime) {
        let mut state = self.state.lock();
        let source_id = if let Some(job) = state.jobs.get_mut(job_id) {
            job.status = SyncJobStatus::Succeeded;
            job.completed_at = Some(format_timestamp(now));
            job.received_records = outcome.received_records;
            job.accepted_records = outcome.accepted_records;
            job.rejected_records = outcome.rejected_records;
            job.generation_id = Some(outcome.generation.id().to_string());
            Some(job.source_id)
        } else {
            None
        };
        if let Some(source_id) = source_id {
            state.active_by_source.remove(&source_id);
        }
    }

    fn finish_error(&self, job_id: &str, error: BackendError, now: OffsetDateTime) {
        let mut state = self.state.lock();
        let source_id = if let Some(job) = state.jobs.get_mut(job_id) {
            job.status = SyncJobStatus::Failed;
            job.completed_at = Some(format_timestamp(now));
            job.error = Some(JobError {
                code: error.code,
                message: error.message,
            });
            Some(job.source_id)
        } else {
            None
        };
        if let Some(source_id) = source_id {
            state.active_by_source.remove(&source_id);
        }
    }

    fn get(&self, job_id: &str) -> Option<SyncJob> {
        self.state.lock().jobs.get(job_id).cloned()
    }
}

pub struct ContractBackend {
    services: Arc<BTreeMap<u16, Arc<ContractService>>>,
    caches: Arc<BTreeMap<u16, Arc<SourceCache>>>,
    config: BackendConfig,
    readiness: Arc<ReadinessState>,
    jobs: Arc<JobCoordinator>,
}

impl ContractBackend {
    pub fn new(
        services: BTreeMap<u16, Arc<ContractService>>,
        caches: BTreeMap<u16, Arc<SourceCache>>,
        config: BackendConfig,
        readiness: Arc<ReadinessState>,
    ) -> Result<Self, BackendConfigError> {
        if config.max_retained_jobs == 0 {
            return Err(BackendConfigError::ZeroJobCapacity);
        }
        if !services.keys().eq(caches.keys()) {
            return Err(BackendConfigError::RegistryMismatch);
        }
        Ok(Self {
            services: Arc::new(services),
            caches: Arc::new(caches),
            config,
            readiness,
            jobs: Arc::new(JobCoordinator::new(config.max_retained_jobs)),
        })
    }

    async fn source_lookup(
        &self,
        source_id: u16,
        symbol: String,
        consistency: Consistency,
        now: OffsetDateTime,
    ) -> Result<LookupResult<ContractResponse>, BackendError> {
        let service = self.services.get(&source_id).ok_or_else(|| {
            contextual_error(
                ApiErrorCode::UnsupportedSource,
                format!("source {source_id} is not configured"),
                Some(source_id),
                Some(symbol.clone()),
            )
        })?;
        let lookup = service
            .lookup_exact(
                source_id,
                symbol.clone(),
                cache_consistency(consistency),
                now,
            )
            .await
            .map_err(|error| map_service_error(error, Some(source_id), Some(symbol)))?;
        let generation_id = self
            .caches
            .get(&source_id)
            .expect("source registries were validated")
            .snapshot()
            .id();
        lookup_result(lookup, generation_id, self.config.freshness, now)
    }
}

#[salvo::async_trait]
impl ContractHttpBackend for ContractBackend {
    async fn get_by_source(
        &self,
        lookup: SourceLookup,
    ) -> Result<LookupResult<ContractResponse>, BackendError> {
        self.source_lookup(
            lookup.source_id,
            lookup.symbol,
            lookup.consistency,
            OffsetDateTime::now_utc(),
        )
        .await
    }

    async fn get_by_exchange(
        &self,
        lookup: ExchangeLookup,
    ) -> Result<LookupResult<ExchangeContractResponse>, BackendError> {
        let now = OffsetDateTime::now_utc();
        let mut keys = Vec::new();
        for (source_id, cache) in self.caches.iter() {
            for contract in cache.by_exchange_code(&lookup.exchange, &lookup.code) {
                keys.push((*source_id, contract.metadata.key.symbol.clone()));
            }
        }
        keys.sort();
        keys.dedup();
        if keys.is_empty() {
            let mut error = BackendError::new(
                ApiErrorCode::ContractNotFound,
                "no contract matches the exchange and code",
            );
            error.context.exchange = Some(lookup.exchange);
            error.context.code = Some(lookup.code);
            return Err(error);
        }

        let mut results = Vec::with_capacity(keys.len());
        for (source_id, symbol) in keys {
            results.push(
                self.source_lookup(source_id, symbol, lookup.consistency, now)
                    .await?,
            );
        }
        results.sort_by(|left, right| {
            left.body
                .data
                .symbol
                .cmp(&right.body.data.symbol)
                .then(left.body.data.source_id.cmp(&right.body.data.source_id))
        });
        let last_modified = results
            .iter()
            .filter_map(|result| {
                self.caches
                    .get(&result.body.data.source_id)?
                    .get(&result.body.data.symbol)
                    .map(|contract| contract.metadata.metadata_observed_at)
            })
            .max()
            .map(format_http_date)
            .unwrap_or_else(|| "Thu, 01 Jan 1970 00:00:00 GMT".to_owned());
        let matches: Vec<_> = results.into_iter().map(|result| result.body).collect();
        let body = ExchangeContractResponse {
            match_count: matches.len(),
            matches,
        };
        Ok(LookupResult {
            etag: exchange_etag(&body)?,
            last_modified,
            body,
        })
    }

    async fn readiness(&self) -> HealthResponse {
        let cfapi_session = health(self.readiness.cfapi_session.load(Ordering::Acquire));
        let cache_ready = self.readiness.cache.load(Ordering::Acquire)
            && self
                .caches
                .values()
                .all(|cache| cache.snapshot().is_complete());
        let cache = health(cache_ready);
        let query_coordinator = health(self.readiness.query_coordinator.load(Ordering::Acquire));
        let sync_coordinator = health(self.readiness.sync_coordinator.load(Ordering::Acquire));
        let status = health(
            cfapi_session == ComponentHealth::Ready
                && cache == ComponentHealth::Ready
                && query_coordinator == ComponentHealth::Ready
                && sync_coordinator == ComponentHealth::Ready,
        );
        HealthResponse {
            status,
            cfapi_session,
            cache,
            query_coordinator,
            sync_coordinator,
        }
    }

    async fn create_sync_job(
        &self,
        request: CreateSyncJobRequest,
        idempotency_key: Option<String>,
    ) -> Result<CreatedSyncJob, BackendError> {
        let service = Arc::clone(self.services.get(&request.source_id).ok_or_else(|| {
            contextual_error(
                ApiErrorCode::UnsupportedSource,
                format!("source {} is not configured", request.source_id),
                Some(request.source_id),
                None,
            )
        })?);
        if !self.readiness.sync_coordinator.load(Ordering::Acquire) {
            let mut error = BackendError::new(
                ApiErrorCode::NotReady,
                "synchronization coordinator is not ready",
            );
            error.retryable = true;
            return Err(error);
        }
        let created = self.jobs.create(
            request.source_id,
            idempotency_key,
            OffsetDateTime::now_utc(),
        )?;
        if created.replayed {
            return Ok(created);
        }
        let job_id = created.job.job_id.clone();
        let jobs = Arc::clone(&self.jobs);
        tokio::spawn(async move {
            jobs.mark_running(&job_id, OffsetDateTime::now_utc());
            match service.sync_source(request.source_id).await {
                Ok(outcome) => jobs.finish_success(&job_id, outcome, OffsetDateTime::now_utc()),
                Err(error) => jobs.finish_error(
                    &job_id,
                    map_service_error(error, Some(request.source_id), None),
                    OffsetDateTime::now_utc(),
                ),
            }
        });
        Ok(created)
    }

    async fn get_sync_job(&self, job_id: String) -> Result<SyncJob, BackendError> {
        self.jobs.get(&job_id).ok_or_else(|| {
            BackendError::new(
                ApiErrorCode::SyncJobNotFound,
                format!("synchronization job {job_id} was not found"),
            )
        })
    }
}

fn lookup_result(
    lookup: ContractLookup,
    generation_id: Ulid,
    policy: FreshnessPolicy,
    now: OffsetDateTime,
) -> Result<LookupResult<ContractResponse>, BackendError> {
    let observed_at = lookup.contract.metadata.metadata_observed_at;
    let body = ContractResponse {
        data: contract_dto(&lookup.contract),
        freshness: freshness_dto(
            &lookup.contract,
            lookup.freshness,
            generation_id,
            policy,
            now,
        ),
    };
    Ok(LookupResult {
        etag: contract_etag(&body)?,
        last_modified: format_http_date(observed_at),
        body,
    })
}

fn contract_dto(view: &ContractView) -> Contract {
    let dto = DomainContract::from_view(view);
    Contract {
        schema_version: CONTRACT_SCHEMA_VERSION,
        source_id: dto.source_id,
        exchange: dto.exchange,
        feed_mic: dto.feed_mic,
        code: dto.code,
        symbol: dto.symbol,
        name: dto.name,
        category: dto.category,
        sector: dto.sector,
        industry: dto.industry,
        unit: dto.unit,
        unit_value: dto.unit_value,
        reference: dto.reference,
        reference_observed_at: dto.reference_observed_at,
        currency: dto.currency,
        instrument_type_code: dto.instrument_type_code,
        instrument_type: dto.instrument_type,
        tick_size: dto.tick_size,
        tick_size_rules: dto.tick_size_rules.map(|rules| {
            rules
                .into_iter()
                .map(|rule| TickSizeRule {
                    tick: rule.tick,
                    upper_bound: rule.upper_bound,
                })
                .collect()
        }),
        update_date: Some(dto.update_date),
        metadata_observed_at: dto.metadata_observed_at,
    }
}

fn freshness_dto(
    view: &ContractView,
    decision: FreshnessDecision,
    generation_id: Ulid,
    policy: FreshnessPolicy,
    now: OffsetDateTime,
) -> Freshness {
    let observed_at = view.metadata.metadata_observed_at;
    let metadata_age_seconds = age_seconds(observed_at, now);
    let fresh_until = observed_at + Duration::seconds(policy.fresh_for.min(i64::MAX as u64) as i64);
    let (metadata, stale_since, served_stale_due_to) = match decision {
        FreshnessDecision::Fresh => (FreshnessState::Fresh, None, None),
        FreshnessDecision::Stale(reason) => (
            FreshnessState::Stale,
            Some(format_timestamp(fresh_until)),
            Some(refresh_reason(reason).to_owned()),
        ),
        FreshnessDecision::Unavailable => (FreshnessState::Absent, None, None),
    };
    let (reference, reference_age_seconds) =
        view.reference
            .as_ref()
            .map_or((FreshnessState::Absent, None), |reference| {
                let age = age_seconds(reference.observed_at, now);
                let state = if age <= policy.fresh_for {
                    FreshnessState::Fresh
                } else {
                    FreshnessState::Stale
                };
                (state, Some(age))
            });
    Freshness {
        generation_id: generation_id.to_string(),
        metadata,
        metadata_age_seconds,
        fresh_until: format_timestamp(fresh_until),
        stale_since,
        reference,
        reference_age_seconds,
        served_stale_due_to,
    }
}

#[derive(Serialize)]
struct CanonicalEtagEntry<'a> {
    data: &'a Contract,
    generation_id: &'a str,
    metadata: FreshnessState,
    reference: FreshnessState,
    served_stale_due_to: Option<&'a str>,
}

fn contract_etag(body: &ContractResponse) -> Result<String, BackendError> {
    hash_etag(&CanonicalEtagEntry {
        data: &body.data,
        generation_id: &body.freshness.generation_id,
        metadata: body.freshness.metadata,
        reference: body.freshness.reference,
        served_stale_due_to: body.freshness.served_stale_due_to.as_deref(),
    })
}

fn exchange_etag(body: &ExchangeContractResponse) -> Result<String, BackendError> {
    let entries: Vec<_> = body
        .matches
        .iter()
        .map(|item| CanonicalEtagEntry {
            data: &item.data,
            generation_id: &item.freshness.generation_id,
            metadata: item.freshness.metadata,
            reference: item.freshness.reference,
            served_stale_due_to: item.freshness.served_stale_due_to.as_deref(),
        })
        .collect();
    hash_etag(&entries)
}

fn hash_etag<T: Serialize>(content: &T) -> Result<String, BackendError> {
    let bytes = rmp_serde::to_vec_named(content).map_err(|_| {
        BackendError::new(
            ApiErrorCode::InternalError,
            "failed to derive response ETag",
        )
    })?;
    let hash = bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    Ok(format!("\"cq-v1-{hash:016x}\""))
}

fn map_service_error(
    error: ServiceError,
    source_id: Option<u16>,
    symbol: Option<String>,
) -> BackendError {
    let (code, message, retryable) = match error {
        ServiceError::UnsupportedSource(source_id) => (
            ApiErrorCode::UnsupportedSource,
            format!("source {source_id} is not configured"),
            false,
        ),
        ServiceError::InvalidRow(_) => (
            ApiErrorCode::UpstreamProtocolError,
            "upstream returned an invalid contract row".to_owned(),
            false,
        ),
        ServiceError::Cache(_) | ServiceError::Staging(_) => (
            ApiErrorCode::InternalError,
            "contract cache update failed".to_owned(),
            false,
        ),
        ServiceError::Query(query) => map_query_error(query),
    };
    let mut backend = BackendError::new(code, message);
    backend.retryable = retryable;
    backend.context.source_id = source_id;
    backend.context.symbol = symbol;
    backend
}

fn map_query_error(error: QueryError) -> (ApiErrorCode, String, bool) {
    match error {
        QueryError::NotFound => (
            ApiErrorCode::ContractNotFound,
            "contract was not found".to_owned(),
            false,
        ),
        QueryError::PermissionDenied => (
            ApiErrorCode::PermissionDenied,
            "source permission was denied".to_owned(),
            false,
        ),
        QueryError::Timeout => (
            ApiErrorCode::UpstreamTimeout,
            "upstream query timed out".to_owned(),
            true,
        ),
        QueryError::SessionUnavailable | QueryError::Cancelled => (
            ApiErrorCode::UpstreamUnavailable,
            "upstream session is unavailable".to_owned(),
            true,
        ),
        QueryError::ShuttingDown => (
            ApiErrorCode::ShuttingDown,
            "query coordinator is shutting down".to_owned(),
            true,
        ),
        QueryError::LocalBackpressure
        | QueryError::SendQueueFull
        | QueryError::ResponseBackpressure
        | QueryError::GenerationLimit
        | QueryError::TagCollision => (
            ApiErrorCode::Backpressure,
            "query capacity is temporarily unavailable".to_owned(),
            true,
        ),
        QueryError::CfapiStatus { code, .. } => (
            ApiErrorCode::UpstreamStatus,
            format!("upstream query failed with status {code}"),
            false,
        ),
        QueryError::ProtocolViolation(_) => (
            ApiErrorCode::UpstreamProtocolError,
            "upstream query violated the QueryXref protocol".to_owned(),
            false,
        ),
    }
}

fn contextual_error(
    code: ApiErrorCode,
    message: String,
    source_id: Option<u16>,
    symbol: Option<String>,
) -> BackendError {
    BackendError {
        code,
        message,
        retryable: false,
        context: Box::new(ErrorContext {
            source_id,
            symbol,
            ..ErrorContext::default()
        }),
    }
}

fn cache_consistency(consistency: Consistency) -> CacheConsistency {
    match consistency {
        Consistency::CachePreferred => CacheConsistency::CachePreferred,
        Consistency::FreshRequired => CacheConsistency::FreshRequired,
    }
}

fn refresh_reason(reason: RefreshFailure) -> &'static str {
    match reason {
        RefreshFailure::SessionUnavailable => "upstream_unavailable",
        RefreshFailure::Timeout => "refresh_timeout",
        RefreshFailure::Backpressure => "backpressure",
        RefreshFailure::RetryableTransport => "retryable_transport",
        RefreshFailure::RefreshFailed => "refresh_failed",
        RefreshFailure::PermissionDenied => "permission_denied",
        RefreshFailure::UnsupportedSource => "unsupported_source",
        RefreshFailure::NotFound => "not_found",
        RefreshFailure::ProtocolViolation => "protocol_violation",
        RefreshFailure::ValidationFailed => "validation_failed",
    }
}

fn age_seconds(observed_at: OffsetDateTime, now: OffsetDateTime) -> u64 {
    now.unix_timestamp()
        .saturating_sub(observed_at.unix_timestamp())
        .max(0) as u64
}

fn health(ready: bool) -> ComponentHealth {
    if ready {
        ComponentHealth::Ready
    } else {
        ComponentHealth::NotReady
    }
}

fn format_timestamp(value: OffsetDateTime) -> String {
    value
        .to_offset(UtcOffset::UTC)
        .format(&Rfc3339)
        .expect("OffsetDateTime supports RFC 3339")
}

fn format_http_date(value: OffsetDateTime) -> String {
    let format = time::macros::format_description!(
        "[weekday repr:short], [day padding:zero] [month repr:short] [year] [hour]:[minute]:[second] GMT"
    );
    value
        .to_offset(UtcOffset::UTC)
        .format(&format)
        .expect("OffsetDateTime supports HTTP date formatting")
}
