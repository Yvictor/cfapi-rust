#[path = "../src/http.rs"]
mod http;

use http::*;
use salvo::http::StatusCode;
use salvo::test::{ResponseExt, TestClient};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct MockBackend {
    source_lookups: Mutex<Vec<SourceLookup>>,
    exchange_lookups: Mutex<Vec<ExchangeLookup>>,
    ready: AtomicBool,
}

impl MockBackend {
    fn ready() -> Arc<Self> {
        Arc::new(Self {
            ready: AtomicBool::new(true),
            ..Self::default()
        })
    }
}

#[salvo::async_trait]
impl ContractHttpBackend for MockBackend {
    async fn get_by_source(
        &self,
        lookup: SourceLookup,
    ) -> Result<LookupResult<ContractResponse>, BackendError> {
        self.source_lookups.lock().unwrap().push(lookup.clone());
        match lookup.symbol.as_str() {
            "UNKNOWN" => Err(BackendError::new(
                ApiErrorCode::ContractNotFound,
                "contract was not found",
            )),
            "TIMEOUT" => Err(BackendError {
                retryable: true,
                ..BackendError::new(ApiErrorCode::UpstreamTimeout, "upstream query timed out")
            }),
            _ => Ok(lookup_result(contract_response(
                lookup.source_id,
                &lookup.symbol,
                "XNGS",
            ))),
        }
    }

    async fn get_by_exchange(
        &self,
        lookup: ExchangeLookup,
    ) -> Result<LookupResult<ExchangeContractResponse>, BackendError> {
        self.exchange_lookups.lock().unwrap().push(lookup.clone());
        if lookup.code == "UNKNOWN" {
            return Err(BackendError::new(
                ApiErrorCode::ContractNotFound,
                "contract was not found",
            ));
        }
        let count = if lookup.code == "DUAL" { 2 } else { 1 };
        let matches = (0..count)
            .map(|offset| contract_response(533 + offset, &lookup.code, &lookup.exchange))
            .collect::<Vec<_>>();
        Ok(lookup_result(ExchangeContractResponse {
            match_count: matches.len(),
            matches,
        }))
    }

    async fn readiness(&self) -> HealthResponse {
        let component = if self.ready.load(Ordering::Relaxed) {
            ComponentHealth::Ready
        } else {
            ComponentHealth::NotReady
        };
        HealthResponse {
            status: component,
            cfapi_session: component,
            cache: ComponentHealth::Ready,
            query_coordinator: component,
            sync_coordinator: ComponentHealth::Ready,
        }
    }

    async fn create_sync_job(
        &self,
        request: CreateSyncJobRequest,
        _idempotency_key: Option<String>,
    ) -> Result<CreatedSyncJob, BackendError> {
        Ok(CreatedSyncJob {
            job: sync_job(request.source_id),
            replayed: false,
        })
    }

    async fn get_sync_job(&self, _job_id: String) -> Result<SyncJob, BackendError> {
        Ok(sync_job(533))
    }
}

#[tokio::test]
async fn source_lookup_decodes_reserved_symbol_and_preserves_request_id() {
    let backend = MockBackend::ready();
    let request_id = "01K2Q16A8J9ZP5J8M1X1G3G6Y7";
    let mut response = TestClient::get(
        "http://localhost/v1/contracts/by-source?source_id=533&symbol=A.B%20C%2FD%5CE%25F&consistency=fresh_required",
    )
    .add_header("x-request-id", request_id, true)
    .send(router(backend.clone()))
    .await;

    assert_eq!(response.status_code, Some(StatusCode::OK));
    assert_eq!(response.headers()["x-request-id"], request_id);
    assert_eq!(response.headers()["etag"], "\"contract-v1\"");
    assert_eq!(response.headers()["cache-control"], "private, no-cache");
    let body = response.take_json::<ContractResponse>().await.unwrap();
    assert_eq!(body.data.symbol, "A.B C/D\\E%F");
    assert_eq!(
        backend.source_lookups.lock().unwrap()[0],
        SourceLookup {
            source_id: 533,
            symbol: "A.B C/D\\E%F".into(),
            consistency: Consistency::FreshRequired,
        }
    );

    let mut not_modified =
        TestClient::get("http://localhost/v1/contracts/by-source?source_id=533&symbol=AAPL")
            .add_header("if-none-match", "\"contract-v1\"", true)
            .send(router(backend))
            .await;
    assert_eq!(not_modified.status_code, Some(StatusCode::NOT_MODIFIED));
    assert_eq!(not_modified.headers()["etag"], "\"contract-v1\"");
    assert_eq!(not_modified.headers()["cache-control"], "private, no-cache");
    assert!(not_modified.headers().contains_key("x-request-id"));
    assert!(not_modified.take_bytes(None).await.unwrap().is_empty());
}

#[tokio::test]
async fn lookup_validation_and_backend_errors_use_typed_envelope() {
    let backend = MockBackend::ready();
    let mut invalid =
        TestClient::get("http://localhost/v1/contracts/by-source?source_id=0&symbol=AAPL")
            .send(router(backend.clone()))
            .await;
    assert_eq!(invalid.status_code, Some(StatusCode::BAD_REQUEST));
    let invalid_body = invalid.take_json::<ApiError>().await.unwrap();
    assert_eq!(invalid_body.code, ApiErrorCode::InvalidSourceId);
    assert_eq!(invalid_body.request_id.len(), 26);

    let mut timeout =
        TestClient::get("http://localhost/v1/contracts/by-source?source_id=533&symbol=TIMEOUT")
            .send(router(backend))
            .await;
    assert_eq!(timeout.status_code, Some(StatusCode::GATEWAY_TIMEOUT));
    let timeout_body = timeout.take_json::<ApiError>().await.unwrap();
    assert_eq!(timeout_body.code, ApiErrorCode::UpstreamTimeout);
    assert!(timeout_body.retryable);
}

#[tokio::test]
async fn exchange_lookup_normalizes_mic_and_returns_all_matches() {
    let backend = MockBackend::ready();
    let mut response =
        TestClient::get("http://localhost/v1/contracts/by-exchange?exchange=xngs&code=DUAL")
            .send(router(backend.clone()))
            .await;

    assert_eq!(response.status_code, Some(StatusCode::OK));
    let body = response
        .take_json::<ExchangeContractResponse>()
        .await
        .unwrap();
    assert_eq!(body.match_count, 2);
    assert_eq!(body.matches[0].data.source_id, 533);
    assert_eq!(body.matches[1].data.source_id, 534);
    assert_eq!(backend.exchange_lookups.lock().unwrap()[0].exchange, "XNGS");

    let mut not_modified =
        TestClient::get("http://localhost/v1/contracts/by-exchange?exchange=XNGS&code=AAPL")
            .add_header("if-none-match", "\"contract-v1\"", true)
            .send(router(backend))
            .await;
    assert_eq!(not_modified.status_code, Some(StatusCode::NOT_MODIFIED));
    assert_eq!(not_modified.headers()["etag"], "\"contract-v1\"");
    assert_eq!(not_modified.headers()["cache-control"], "private, no-cache");
    assert!(not_modified.headers().contains_key("x-request-id"));
    assert!(not_modified.take_bytes(None).await.unwrap().is_empty());
}

#[tokio::test]
async fn framework_404_and_405_use_api_error_envelope() {
    let service = service(MockBackend::ready());
    let mut missing = TestClient::get("http://localhost/not-a-route")
        .send(&service)
        .await;
    assert_eq!(missing.status_code, Some(StatusCode::NOT_FOUND));
    assert!(missing.headers().contains_key("x-request-id"));
    let missing_body = missing.take_json::<ApiError>().await.unwrap();
    assert_eq!(missing_body.code, ApiErrorCode::InvalidRequest);
    assert_eq!(missing_body.message, "route was not found");

    let mut wrong_method =
        TestClient::post("http://localhost/v1/contracts/by-source?source_id=533&symbol=AAPL")
            .send(&service)
            .await;
    assert_eq!(
        wrong_method.status_code,
        Some(StatusCode::METHOD_NOT_ALLOWED)
    );
    assert_eq!(wrong_method.headers()["allow"], "GET");
    assert!(wrong_method.headers().contains_key("x-request-id"));
    let method_body = wrong_method.take_json::<ApiError>().await.unwrap();
    assert_eq!(method_body.code, ApiErrorCode::InvalidRequest);
    assert_eq!(method_body.message, "method is not allowed for this route");
}

#[tokio::test]
async fn health_endpoints_are_local_and_readiness_returns_503() {
    let backend = MockBackend::ready();
    backend.ready.store(false, Ordering::Relaxed);
    let mut live = TestClient::get("http://localhost/health/live")
        .send(router(backend.clone()))
        .await;
    assert_eq!(live.status_code, Some(StatusCode::OK));
    assert!(live.take_json::<HealthResponse>().await.unwrap().is_ready());

    let mut ready = TestClient::get("http://localhost/health/ready")
        .send(router(backend))
        .await;
    assert_eq!(ready.status_code, Some(StatusCode::SERVICE_UNAVAILABLE));
    assert!(!ready
        .take_json::<HealthResponse>()
        .await
        .unwrap()
        .is_ready());
}

#[tokio::test]
async fn sync_routes_forward_typed_contract_without_cfapi_behavior() {
    let backend = MockBackend::ready();
    let mut oversized = TestClient::post("http://localhost/v1/contract-sync-jobs")
        .raw_json(format!(
            "{{\"source_id\":533,\"padding\":\"{}\"}}",
            "x".repeat(8 * 1024)
        ))
        .send(router(backend.clone()))
        .await;
    assert_eq!(oversized.status_code, Some(StatusCode::PAYLOAD_TOO_LARGE));
    assert_eq!(
        oversized.take_json::<ApiError>().await.unwrap().code,
        ApiErrorCode::RequestTooLarge
    );

    let mut created = TestClient::post("http://localhost/v1/contract-sync-jobs")
        .add_header("idempotency-key", "client-request-1", true)
        .json(&CreateSyncJobRequest { source_id: 533 })
        .send(router(backend.clone()))
        .await;
    assert_eq!(created.status_code, Some(StatusCode::ACCEPTED));
    assert_eq!(
        created.headers()["location"],
        "/v1/contract-sync-jobs/01K2Q16A8J9ZP5J8M1X1G3G6Y7"
    );
    assert_eq!(created.take_json::<SyncJob>().await.unwrap().source_id, 533);

    let mut fetched =
        TestClient::get("http://localhost/v1/contract-sync-jobs/01K2Q16A8J9ZP5J8M1X1G3G6Y7")
            .send(router(backend))
            .await;
    assert_eq!(fetched.status_code, Some(StatusCode::OK));
    assert_eq!(
        fetched.take_json::<SyncJob>().await.unwrap().status,
        SyncJobStatus::Queued
    );
}

#[tokio::test]
async fn openapi_is_31_with_stable_operations_and_scalar_is_mounted() {
    let backend = MockBackend::ready();
    let mut spec = TestClient::get("http://localhost/api-doc/openapi.json")
        .send(router(backend.clone()))
        .await;
    assert_eq!(spec.status_code, Some(StatusCode::OK));
    let document = spec.take_json::<serde_json::Value>().await.unwrap();
    assert_eq!(document["openapi"], "3.1.0");
    assert_eq!(
        document["paths"]["/v1/contracts/by-source"]["get"]["operationId"],
        "getContractBySource"
    );
    assert_eq!(
        document["paths"]["/health/ready"]["get"]["operationId"],
        "getReadiness"
    );
    for schema in [
        "Contract",
        "ContractResponse",
        "Freshness",
        "TickSizeRule",
        "SyncJob",
        "HealthResponse",
        "ApiError",
    ] {
        assert!(
            document["components"]["schemas"].get(schema).is_some(),
            "missing {schema}"
        );
    }

    let scalar = TestClient::get("http://localhost/doc")
        .send(router(backend))
        .await;
    assert_eq!(scalar.status_code, Some(StatusCode::OK));
}

fn lookup_result<T>(body: T) -> LookupResult<T> {
    LookupResult {
        body,
        etag: "\"contract-v1\"".into(),
        last_modified: "Tue, 19 Aug 2026 12:34:56 GMT".into(),
    }
}

fn contract_response(source_id: u16, symbol: &str, exchange: &str) -> ContractResponse {
    ContractResponse {
        data: Contract {
            schema_version: 1,
            source_id,
            exchange: Some(exchange.into()),
            feed_mic: Some("XNAS".into()),
            code: symbol.into(),
            symbol: symbol.into(),
            name: Some("Example Corp.".into()),
            category: None,
            sector: None,
            industry: None,
            unit: Some("40".into()),
            unit_value: Some("40".into()),
            reference: None,
            reference_observed_at: None,
            currency: Some("USD".into()),
            instrument_type_code: Some(257),
            instrument_type: Some("COMMON_STOCK".into()),
            tick_size: Some("0.01".into()),
            tick_size_rules: Some(vec![TickSizeRule {
                tick: "0.01".into(),
                upper_bound: None,
            }]),
            update_date: Some("2026-08-19".into()),
            metadata_observed_at: "2026-08-19T12:34:56Z".into(),
        },
        freshness: Freshness {
            generation_id: "01K2Q16A8J9ZP5J8M1X1G3G6Y7".into(),
            metadata: FreshnessState::Fresh,
            metadata_age_seconds: 12,
            fresh_until: "2026-08-19T12:39:56Z".into(),
            stale_since: None,
            reference: FreshnessState::Absent,
            reference_age_seconds: None,
            served_stale_due_to: None,
        },
    }
}

fn sync_job(source_id: u16) -> SyncJob {
    SyncJob {
        job_id: "01K2Q16A8J9ZP5J8M1X1G3G6Y7".into(),
        source_id,
        status: SyncJobStatus::Queued,
        submitted_at: "2026-08-19T12:00:00Z".into(),
        started_at: None,
        completed_at: None,
        received_records: 0,
        accepted_records: 0,
        rejected_records: 0,
        generation_id: None,
        error: None,
    }
}
