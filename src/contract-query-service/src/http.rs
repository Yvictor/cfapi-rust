use salvo::affix_state;
use salvo::catcher::Catcher;
use salvo::http::ParseError;
use salvo::http::{header, HeaderValue, StatusCode};
use salvo::oapi::{endpoint, OpenApi, ToSchema};
use salvo::prelude::*;
use salvo_oapi::scalar::Scalar;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use ulid::Ulid;

const REQUEST_ID_HEADER: &str = "x-request-id";
const MAX_SYMBOL_BYTES: usize = 128;
const MAX_IDEMPOTENCY_KEY_BYTES: usize = 128;
const MAX_SYNC_BODY_BYTES: u64 = 8 * 1024;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Consistency {
    #[default]
    CachePreferred,
    FreshRequired,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceLookup {
    pub source_id: u16,
    pub symbol: String,
    pub consistency: Consistency,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExchangeLookup {
    pub exchange: String,
    pub code: String,
    pub consistency: Consistency,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FreshnessState {
    Fresh,
    Stale,
    Absent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[salvo(schema(name = TickSizeRule))]
pub struct TickSizeRule {
    pub tick: String,
    pub upper_bound: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[salvo(schema(name = Contract))]
pub struct Contract {
    pub schema_version: u16,
    pub source_id: u16,
    pub exchange: Option<String>,
    pub feed_mic: Option<String>,
    pub code: String,
    pub symbol: String,
    pub name: Option<String>,
    pub category: Option<String>,
    pub sector: Option<String>,
    pub industry: Option<String>,
    pub unit: Option<String>,
    pub unit_value: Option<String>,
    pub reference: Option<String>,
    pub reference_observed_at: Option<String>,
    pub currency: Option<String>,
    pub instrument_type_code: Option<i32>,
    pub instrument_type: Option<String>,
    pub tick_size: Option<String>,
    pub tick_size_rules: Option<Vec<TickSizeRule>>,
    pub update_date: Option<String>,
    pub metadata_observed_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[salvo(schema(name = Freshness))]
pub struct Freshness {
    pub generation_id: String,
    pub metadata: FreshnessState,
    pub metadata_age_seconds: u64,
    pub fresh_until: String,
    pub stale_since: Option<String>,
    pub reference: FreshnessState,
    pub reference_age_seconds: Option<u64>,
    pub served_stale_due_to: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[salvo(schema(name = ContractResponse))]
pub struct ContractResponse {
    pub data: Contract,
    pub freshness: Freshness,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
pub struct ExchangeContractResponse {
    pub matches: Vec<ContractResponse>,
    pub match_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LookupResult<T> {
    pub body: T,
    pub etag: String,
    pub last_modified: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SyncJobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
pub struct JobError {
    pub code: ApiErrorCode,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[salvo(schema(name = SyncJob))]
pub struct SyncJob {
    pub job_id: String,
    pub source_id: u16,
    pub status: SyncJobStatus,
    pub submitted_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub received_records: u64,
    pub accepted_records: u64,
    pub rejected_records: u64,
    pub generation_id: Option<String>,
    pub error: Option<JobError>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
pub struct CreateSyncJobRequest {
    pub source_id: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreatedSyncJob {
    pub job: SyncJob,
    pub replayed: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ComponentHealth {
    Ready,
    NotReady,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[salvo(schema(name = HealthResponse))]
pub struct HealthResponse {
    pub status: ComponentHealth,
    pub cfapi_session: ComponentHealth,
    pub cache: ComponentHealth,
    pub query_coordinator: ComponentHealth,
    pub sync_coordinator: ComponentHealth,
}

impl HealthResponse {
    pub fn live() -> Self {
        Self {
            status: ComponentHealth::Ready,
            cfapi_session: ComponentHealth::Ready,
            cache: ComponentHealth::Ready,
            query_coordinator: ComponentHealth::Ready,
            sync_coordinator: ComponentHealth::Ready,
        }
    }

    pub fn is_ready(&self) -> bool {
        self.status == ComponentHealth::Ready
            && self.cfapi_session == ComponentHealth::Ready
            && self.cache == ComponentHealth::Ready
            && self.query_coordinator == ComponentHealth::Ready
            && self.sync_coordinator == ComponentHealth::Ready
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApiErrorCode {
    InvalidRequest,
    InvalidSourceId,
    InvalidSymbol,
    InvalidExchange,
    InvalidCode,
    PermissionDenied,
    ContractNotFound,
    SyncJobNotFound,
    SyncAlreadyRunning,
    IdempotencyConflict,
    RequestTooLarge,
    UnsupportedSource,
    UpstreamStatus,
    UpstreamProtocolError,
    NotReady,
    UpstreamUnavailable,
    Backpressure,
    ShuttingDown,
    UpstreamTimeout,
    InternalError,
    ConfigurationError,
}

impl ApiErrorCode {
    pub fn status(self) -> StatusCode {
        match self {
            Self::InvalidRequest
            | Self::InvalidSourceId
            | Self::InvalidSymbol
            | Self::InvalidExchange
            | Self::InvalidCode => StatusCode::BAD_REQUEST,
            Self::PermissionDenied => StatusCode::FORBIDDEN,
            Self::ContractNotFound | Self::SyncJobNotFound => StatusCode::NOT_FOUND,
            Self::SyncAlreadyRunning | Self::IdempotencyConflict => StatusCode::CONFLICT,
            Self::RequestTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::UnsupportedSource => StatusCode::UNPROCESSABLE_ENTITY,
            Self::UpstreamStatus | Self::UpstreamProtocolError => StatusCode::BAD_GATEWAY,
            Self::NotReady
            | Self::UpstreamUnavailable
            | Self::Backpressure
            | Self::ShuttingDown => StatusCode::SERVICE_UNAVAILABLE,
            Self::UpstreamTimeout => StatusCode::GATEWAY_TIMEOUT,
            Self::InternalError | Self::ConfigurationError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
pub struct ErrorContext {
    pub source_id: Option<u16>,
    pub symbol: Option<String>,
    pub exchange: Option<String>,
    pub code: Option<String>,
    pub active_job_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[salvo(schema(name = ApiError))]
pub struct ApiError {
    pub code: ApiErrorCode,
    pub message: String,
    pub request_id: String,
    pub retryable: bool,
    pub context: ErrorContext,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendError {
    pub code: ApiErrorCode,
    pub message: String,
    pub retryable: bool,
    pub context: Box<ErrorContext>,
}

impl BackendError {
    pub fn new(code: ApiErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retryable: false,
            context: Box::default(),
        }
    }
}

#[salvo::async_trait]
pub trait ContractHttpBackend: Send + Sync + 'static {
    async fn get_by_source(
        &self,
        lookup: SourceLookup,
    ) -> Result<LookupResult<ContractResponse>, BackendError>;

    async fn get_by_exchange(
        &self,
        lookup: ExchangeLookup,
    ) -> Result<LookupResult<ExchangeContractResponse>, BackendError>;

    async fn readiness(&self) -> HealthResponse;

    async fn create_sync_job(
        &self,
        request: CreateSyncJobRequest,
        idempotency_key: Option<String>,
    ) -> Result<CreatedSyncJob, BackendError>;

    async fn get_sync_job(&self, job_id: String) -> Result<SyncJob, BackendError>;
}

#[derive(Clone)]
struct BackendState(Arc<dyn ContractHttpBackend>);

pub fn router(backend: Arc<dyn ContractHttpBackend>) -> Router {
    let api = Router::new()
        .push(Router::with_path("v1/contracts/by-source").get(get_contract_by_source))
        .push(Router::with_path("v1/contracts/by-exchange").get(get_contract_by_exchange))
        .push(Router::with_path("v1/contract-sync-jobs").post(create_contract_sync_job))
        .push(Router::with_path("v1/contract-sync-jobs/{job_id}").get(get_contract_sync_job))
        .push(Router::with_path("health/live").get(get_liveness))
        .push(Router::with_path("health/ready").get(get_readiness));
    let document =
        OpenApi::new("Contract Query Service", env!("CARGO_PKG_VERSION")).merge_router(&api);

    Router::new()
        .hoop(affix_state::inject(BackendState(backend)))
        .push(api)
        .push(document.into_router("/api-doc/openapi.json"))
        .push(Scalar::new("/api-doc/openapi.json").into_router("/doc"))
}

pub fn service(backend: Arc<dyn ContractHttpBackend>) -> Service {
    Service::new(router(backend)).catcher(Catcher::default().hoop(api_error_catcher))
}

#[handler]
async fn api_error_catcher(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
    ctrl: &mut FlowCtrl,
) {
    let Some(status) = res.status_code else {
        ctrl.call_next(req, depot, res).await;
        return;
    };
    if status != StatusCode::NOT_FOUND && status != StatusCode::METHOD_NOT_ALLOWED {
        ctrl.call_next(req, depot, res).await;
        return;
    }

    if status == StatusCode::METHOD_NOT_ALLOWED && !res.headers().contains_key(header::ALLOW) {
        if let Some(methods) = allowed_methods(req.uri().path()) {
            res.headers_mut()
                .insert(header::ALLOW, HeaderValue::from_static(methods));
        }
    }

    let request_id = request_id(req);
    set_request_id(res, &request_id);
    res.render(Json(ApiError {
        code: ApiErrorCode::InvalidRequest,
        message: if status == StatusCode::NOT_FOUND {
            "route was not found".into()
        } else {
            "method is not allowed for this route".into()
        },
        request_id,
        retryable: false,
        context: ErrorContext::default(),
    }));
    ctrl.skip_rest();
}

fn allowed_methods(path: &str) -> Option<&'static str> {
    match path {
        "/v1/contracts/by-source"
        | "/v1/contracts/by-exchange"
        | "/health/live"
        | "/health/ready"
        | "/api-doc/openapi.json"
        | "/doc" => Some("GET"),
        "/v1/contract-sync-jobs" => Some("POST"),
        path if path
            .strip_prefix("/v1/contract-sync-jobs/")
            .is_some_and(|job_id| !job_id.is_empty() && !job_id.contains('/')) =>
        {
            Some("GET")
        }
        _ => None,
    }
}

/// Look up a contract by its canonical CFAPI source key.
#[endpoint(
    operation_id = "getContractBySource",
    tags("Contracts"),
    parameters(
        ("source_id" = u16, Query, minimum = 1, maximum = 65535),
        ("symbol" = String, Query, min_length = 1, max_length = 128),
        ("consistency" = Option<Consistency>, Query)
    ),
    responses(
        (status_code = 200, description = "Contract found", body = ContractResponse),
        (status_code = 400, description = "Invalid lookup parameters", body = ApiError),
        (status_code = 403, description = "Source permission denied", body = ApiError),
        (status_code = 404, description = "Contract not found", body = ApiError),
        (status_code = 422, description = "Source is unsupported", body = ApiError),
        (status_code = 502, description = "Invalid upstream response", body = ApiError),
        (status_code = 503, description = "Service or upstream unavailable", body = ApiError),
        (status_code = 504, description = "Upstream timed out", body = ApiError),
        (status_code = 500, description = "Internal or configuration error", body = ApiError)
    )
)]
async fn get_contract_by_source(req: &mut Request, depot: &Depot, res: &mut Response) {
    let request_id = request_id(req);
    let lookup = match parse_source_lookup(req) {
        Ok(lookup) => lookup,
        Err(error) => return render_error(res, request_id, error),
    };
    let backend = match backend(depot) {
        Ok(backend) => backend,
        Err(error) => return render_error(res, request_id, error),
    };
    match backend.get_by_source(lookup).await {
        Ok(result) => render_lookup(req, res, request_id, result),
        Err(error) => render_error(res, request_id, error),
    }
}

/// Look up every canonical contract matching a listing MIC and code.
#[endpoint(
    operation_id = "getContractByExchange",
    tags("Contracts"),
    parameters(
        ("exchange" = String, Query, min_length = 4, max_length = 4),
        ("code" = String, Query, min_length = 1, max_length = 128),
        ("consistency" = Option<Consistency>, Query)
    ),
    responses(
        (status_code = 200, description = "One or more contracts found", body = ExchangeContractResponse),
        (status_code = 400, description = "Invalid lookup parameters", body = ApiError),
        (status_code = 403, description = "Source permission denied", body = ApiError),
        (status_code = 404, description = "Contract not found", body = ApiError),
        (status_code = 422, description = "Source is unsupported", body = ApiError),
        (status_code = 502, description = "Invalid upstream response", body = ApiError),
        (status_code = 503, description = "Service or upstream unavailable", body = ApiError),
        (status_code = 504, description = "Upstream timed out", body = ApiError),
        (status_code = 500, description = "Internal or configuration error", body = ApiError)
    )
)]
async fn get_contract_by_exchange(req: &mut Request, depot: &Depot, res: &mut Response) {
    let request_id = request_id(req);
    let lookup = match parse_exchange_lookup(req) {
        Ok(lookup) => lookup,
        Err(error) => return render_error(res, request_id, error),
    };
    let backend = match backend(depot) {
        Ok(backend) => backend,
        Err(error) => return render_error(res, request_id, error),
    };
    match backend.get_by_exchange(lookup).await {
        Ok(result) => render_lookup(req, res, request_id, result),
        Err(error) => render_error(res, request_id, error),
    }
}

/// Create an asynchronous whole-source synchronization job.
#[endpoint(
    operation_id = "createContractSyncJob",
    tags("Synchronization"),
    request_body(content = CreateSyncJobRequest, content_type = "application/json"),
    parameters(("Idempotency-Key" = Option<String>, Header, max_length = 128)),
    responses(
        (status_code = 202, description = "Synchronization job accepted", body = SyncJob),
        (status_code = 400, description = "Invalid request", body = ApiError),
        (status_code = 409, description = "Synchronization conflict", body = ApiError),
        (status_code = 413, description = "Request body too large", body = ApiError),
        (status_code = 422, description = "Source is unsupported", body = ApiError),
        (status_code = 503, description = "Coordinator unavailable", body = ApiError),
        (status_code = 500, description = "Internal or configuration error", body = ApiError)
    )
)]
async fn create_contract_sync_job(req: &mut Request, depot: &Depot, res: &mut Response) {
    let request_id = request_id(req);
    let key = match parse_idempotency_key(req) {
        Ok(key) => key,
        Err(error) => return render_error(res, request_id, error),
    };
    let request = match req
        .parse_json_with_max_size::<CreateSyncJobRequest>(MAX_SYNC_BODY_BYTES as usize)
        .await
    {
        Ok(request) if request.source_id != 0 => request,
        Ok(_) => {
            return render_error(
                res,
                request_id,
                BackendError::new(ApiErrorCode::InvalidSourceId, "source_id must be nonzero"),
            )
        }
        Err(ParseError::PayloadTooLarge) => {
            return render_error(
                res,
                request_id,
                BackendError::new(ApiErrorCode::RequestTooLarge, "request body exceeds 8 KiB"),
            )
        }
        Err(_) => {
            return render_error(
                res,
                request_id,
                BackendError::new(
                    ApiErrorCode::InvalidRequest,
                    "request body must be valid JSON",
                ),
            )
        }
    };
    let backend = match backend(depot) {
        Ok(backend) => backend,
        Err(error) => return render_error(res, request_id, error),
    };
    match backend.create_sync_job(request, key).await {
        Ok(created) => {
            set_request_id(res, &request_id);
            res.status_code(StatusCode::ACCEPTED);
            let location = format!("/v1/contract-sync-jobs/{}", created.job.job_id);
            if let Ok(value) = HeaderValue::from_str(&location) {
                res.headers_mut().insert(header::LOCATION, value);
            }
            if created.replayed {
                res.headers_mut().insert(
                    header::HeaderName::from_static("idempotency-replayed"),
                    HeaderValue::from_static("true"),
                );
            }
            res.render(Json(created.job));
        }
        Err(error) => render_error(res, request_id, error),
    }
}

/// Get the current state of a synchronization job.
#[endpoint(
    operation_id = "getContractSyncJob",
    tags("Synchronization"),
    parameters(("job_id" = String, Path, min_length = 26, max_length = 26)),
    responses(
        (status_code = 200, description = "Synchronization job", body = SyncJob),
        (status_code = 400, description = "Invalid job ID", body = ApiError),
        (status_code = 404, description = "Synchronization job not found", body = ApiError),
        (status_code = 500, description = "Internal or configuration error", body = ApiError)
    )
)]
async fn get_contract_sync_job(req: &mut Request, depot: &Depot, res: &mut Response) {
    let request_id = request_id(req);
    let job_id = req.param::<String>("job_id").unwrap_or_default();
    if job_id.parse::<Ulid>().is_err() {
        return render_error(
            res,
            request_id,
            BackendError::new(ApiErrorCode::InvalidRequest, "job_id must be a ULID"),
        );
    }
    let backend = match backend(depot) {
        Ok(backend) => backend,
        Err(error) => return render_error(res, request_id, error),
    };
    match backend.get_sync_job(job_id).await {
        Ok(job) => {
            set_request_id(res, &request_id);
            res.render(Json(job));
        }
        Err(error) => render_error(res, request_id, error),
    }
}

/// Report whether the HTTP process is responsive.
#[endpoint(
    operation_id = "getLiveness",
    tags("Health"),
    responses((status_code = 200, description = "Process is live", body = HealthResponse))
)]
async fn get_liveness(req: &Request, res: &mut Response) {
    let request_id = request_id(req);
    set_request_id(res, &request_id);
    res.render(Json(HealthResponse::live()));
}

/// Report whether every local service component can accept work.
#[endpoint(
    operation_id = "getReadiness",
    tags("Health"),
    responses(
        (status_code = 200, description = "Service is ready", body = HealthResponse),
        (status_code = 503, description = "Service is not ready", body = HealthResponse)
    )
)]
async fn get_readiness(req: &Request, depot: &Depot, res: &mut Response) {
    let request_id = request_id(req);
    let backend = match backend(depot) {
        Ok(backend) => backend,
        Err(error) => return render_error(res, request_id, error),
    };
    let health = backend.readiness().await;
    set_request_id(res, &request_id);
    if !health.is_ready() {
        res.status_code(StatusCode::SERVICE_UNAVAILABLE);
    }
    res.render(Json(health));
}

fn backend(depot: &Depot) -> Result<&dyn ContractHttpBackend, BackendError> {
    depot
        .get_typed::<BackendState>()
        .map(|state| state.0.as_ref())
        .map_err(|_| {
            BackendError::new(
                ApiErrorCode::ConfigurationError,
                "HTTP backend is not configured",
            )
        })
}

fn parse_source_lookup(req: &Request) -> Result<SourceLookup, BackendError> {
    let source_id = req
        .query::<u16>("source_id")
        .filter(|value| *value != 0)
        .ok_or_else(|| {
            BackendError::new(
                ApiErrorCode::InvalidSourceId,
                "source_id must be an integer from 1 through 65535",
            )
        })?;
    let symbol = required_text(req, "symbol", ApiErrorCode::InvalidSymbol)?;
    Ok(SourceLookup {
        source_id,
        symbol,
        consistency: consistency(req)?,
    })
}

fn parse_exchange_lookup(req: &Request) -> Result<ExchangeLookup, BackendError> {
    let exchange = req
        .query::<String>("exchange")
        .map(|value| value.to_ascii_uppercase())
        .filter(|value| value.len() == 4 && value.bytes().all(|byte| byte.is_ascii_alphanumeric()))
        .ok_or_else(|| {
            BackendError::new(
                ApiErrorCode::InvalidExchange,
                "exchange must be four ASCII alphanumeric characters",
            )
        })?;
    let code = required_text(req, "code", ApiErrorCode::InvalidCode)?;
    Ok(ExchangeLookup {
        exchange,
        code,
        consistency: consistency(req)?,
    })
}

fn consistency(req: &Request) -> Result<Consistency, BackendError> {
    match req.query::<String>("consistency").as_deref() {
        None | Some("cache_preferred") => Ok(Consistency::CachePreferred),
        Some("fresh_required") => Ok(Consistency::FreshRequired),
        Some(_) => Err(BackendError::new(
            ApiErrorCode::InvalidRequest,
            "consistency must be cache_preferred or fresh_required",
        )),
    }
}

fn required_text(req: &Request, name: &str, code: ApiErrorCode) -> Result<String, BackendError> {
    let value = req.query::<String>(name).unwrap_or_default();
    let valid = !value.is_empty()
        && value.len() <= MAX_SYMBOL_BYTES
        && !value
            .chars()
            .any(|character| character == '\0' || character.is_control());
    valid.then_some(value).ok_or_else(|| {
        BackendError::new(
            code,
            format!("{name} must be 1-128 UTF-8 bytes without control characters"),
        )
    })
}

fn parse_idempotency_key(req: &Request) -> Result<Option<String>, BackendError> {
    let Some(value) = req.headers().get("idempotency-key") else {
        return Ok(None);
    };
    let value = value.to_str().map_err(|_| {
        BackendError::new(
            ApiErrorCode::InvalidRequest,
            "Idempotency-Key must be visible ASCII",
        )
    })?;
    let valid = !value.is_empty()
        && value.len() <= MAX_IDEMPOTENCY_KEY_BYTES
        && value.bytes().all(|byte| (0x21..=0x7e).contains(&byte));
    valid.then(|| Some(value.to_owned())).ok_or_else(|| {
        BackendError::new(
            ApiErrorCode::InvalidRequest,
            "Idempotency-Key must be 1-128 visible ASCII characters",
        )
    })
}

fn request_id(req: &Request) -> String {
    req.headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .filter(|value| value.parse::<Ulid>().is_ok())
        .map(str::to_owned)
        .unwrap_or_else(|| Ulid::new().to_string())
}

fn set_request_id(res: &mut Response, request_id: &str) {
    if let Ok(value) = HeaderValue::from_str(request_id) {
        res.headers_mut()
            .insert(header::HeaderName::from_static(REQUEST_ID_HEADER), value);
    }
}

fn render_lookup<T: Serialize + Send>(
    req: &Request,
    res: &mut Response,
    request_id: String,
    result: LookupResult<T>,
) {
    set_request_id(res, &request_id);
    if let Ok(value) = HeaderValue::from_str(&result.etag) {
        res.headers_mut().insert(header::ETAG, value);
    }
    if let Ok(value) = HeaderValue::from_str(&result.last_modified) {
        res.headers_mut().insert(header::LAST_MODIFIED, value);
    }
    res.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-cache"),
    );
    if req
        .headers()
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .any(|value| value == result.etag)
    {
        res.status_code(StatusCode::NOT_MODIFIED);
        return;
    }
    res.render(Json(result.body));
}

fn render_error(res: &mut Response, request_id: String, error: BackendError) {
    set_request_id(res, &request_id);
    res.status_code(error.code.status());
    res.render(Json(ApiError {
        code: error.code,
        message: error.message,
        request_id,
        retryable: error.retryable,
        context: *error.context,
    }));
}
