pub use contract_query_service::{cache, domain, http, query, service};

#[path = "../src/backend.rs"]
mod backend;

use backend::{BackendConfig, ContractBackend, ReadinessState};
use contract_query_service::{
    cache::{CacheLimits, FreshnessPolicy, SourceCache},
    http::{
        ApiErrorCode, Consistency, ContractHttpBackend, CreateSyncJobRequest, ExchangeLookup,
        SourceLookup, SyncJobStatus,
    },
    query::{PendingRegistry, QueryError, QueryTag, RegistryLimits},
    service::{CfapiCommandOwner, ContractService, OwnerExecuteError, OwnerQuery, ServiceConfig},
    OwnedQueryXrefRow, OwnedToken, OwnedTokenValue,
};
use std::{
    collections::{BTreeMap, VecDeque},
    num::NonZeroI64,
    sync::{
        atomic::{AtomicI64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use time::OffsetDateTime;
use ulid::Ulid;

enum Action {
    Whole(Vec<OwnedQueryXrefRow>),
    NoCallback,
}

struct FakeOwner {
    registry: Arc<PendingRegistry>,
    actions: Arc<Mutex<VecDeque<Action>>>,
    next_tag: AtomicI64,
}

impl CfapiCommandOwner for FakeOwner {
    fn execute(
        &mut self,
        _query: &OwnerQuery,
        bind: &mut dyn FnMut(QueryTag) -> Result<(), QueryError>,
    ) -> Result<QueryTag, OwnerExecuteError> {
        let tag = NonZeroI64::new(self.next_tag.fetch_add(1, Ordering::Relaxed)).unwrap();
        bind(tag)?;
        match self
            .actions
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Action::NoCallback)
        {
            Action::Whole(rows) => {
                for row in rows {
                    self.registry.on_image_part(tag, row);
                }
                self.registry.on_image_complete(tag, None);
            }
            Action::NoCallback => {}
        }
        Ok(tag)
    }
}

struct SourceFixture {
    service: Arc<ContractService>,
    cache: Arc<SourceCache>,
    actions: Arc<Mutex<VecDeque<Action>>>,
}

fn source_fixture(source_id: u16, timeout: Duration) -> SourceFixture {
    let cache = Arc::new(SourceCache::new(source_id, CacheLimits::default()));
    let registry = Arc::new(PendingRegistry::new(RegistryLimits::default()));
    let actions = Arc::new(Mutex::new(VecDeque::new()));
    let owner = FakeOwner {
        registry: Arc::clone(&registry),
        actions: Arc::clone(&actions),
        next_tag: AtomicI64::new(1),
    };
    let service = Arc::new(ContractService::new(
        Arc::clone(&cache),
        registry,
        owner,
        ServiceConfig {
            freshness: policy(),
            query_timeout: timeout,
            negative_ttl: 30,
            command_capacity: 8,
        },
    ));
    SourceFixture {
        service,
        cache,
        actions,
    }
}

fn policy() -> FreshnessPolicy {
    FreshnessPolicy {
        fresh_for: 3_600,
        max_stale_for: 86_400,
    }
}

fn row(source_id: u16, symbol: &str, exchange: &str, name: &str) -> OwnedQueryXrefRow {
    OwnedQueryXrefRow {
        source_id,
        symbol: symbol.to_owned(),
        tokens: vec![
            OwnedToken {
                number: 5,
                value: OwnedTokenValue::String(symbol.to_owned()),
            },
            OwnedToken {
                number: 3960,
                value: OwnedTokenValue::String(name.to_owned()),
            },
            OwnedToken {
                number: 3241,
                value: OwnedTokenValue::String(exchange.to_owned()),
            },
            OwnedToken {
                number: 435,
                value: OwnedTokenValue::String("USD".to_owned()),
            },
        ],
        observed_at: OffsetDateTime::now_utc(),
    }
}

fn publish(cache: &SourceCache, rows: Vec<OwnedQueryXrefRow>) {
    let mut staging = cache.begin_sync();
    for row in rows {
        let contract = contract_query_service::convert_query_xref_row(row, None)
            .unwrap()
            .contract;
        staging.push(Arc::new(contract)).unwrap();
    }
    cache.commit_sync(staging).unwrap();
}

fn backend(fixtures: &[&SourceFixture], max_retained_jobs: usize) -> ContractBackend {
    backend_with_readiness(
        fixtures,
        max_retained_jobs,
        Arc::new(ReadinessState::ready()),
    )
}

fn backend_with_readiness(
    fixtures: &[&SourceFixture],
    max_retained_jobs: usize,
    readiness: Arc<ReadinessState>,
) -> ContractBackend {
    let services = fixtures
        .iter()
        .map(|fixture| {
            (
                fixture.cache.snapshot().source_id(),
                Arc::clone(&fixture.service),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let caches = fixtures
        .iter()
        .map(|fixture| {
            (
                fixture.cache.snapshot().source_id(),
                Arc::clone(&fixture.cache),
            )
        })
        .collect::<BTreeMap<_, _>>();
    ContractBackend::new(
        services,
        caches,
        BackendConfig {
            freshness: policy(),
            max_retained_jobs,
        },
        readiness,
    )
    .unwrap()
}

#[tokio::test]
async fn same_symbol_remains_distinct_across_sources() {
    let left = source_fixture(533, Duration::from_millis(50));
    let right = source_fixture(534, Duration::from_millis(50));
    publish(&left.cache, vec![row(533, "AAPL", "XNAS", "left")]);
    publish(&right.cache, vec![row(534, "AAPL", "XNAS", "right")]);
    let backend = backend(&[&left, &right], 8);

    let left_result = backend
        .get_by_source(SourceLookup {
            source_id: 533,
            symbol: "AAPL".into(),
            consistency: Consistency::CachePreferred,
        })
        .await
        .unwrap();
    let right_result = backend
        .get_by_source(SourceLookup {
            source_id: 534,
            symbol: "AAPL".into(),
            consistency: Consistency::CachePreferred,
        })
        .await
        .unwrap();

    assert_eq!(left_result.body.data.name.as_deref(), Some("left"));
    assert_eq!(right_result.body.data.name.as_deref(), Some("right"));
    assert_ne!(left_result.etag, right_result.etag);
    let generation_id = &left_result.body.freshness.generation_id;
    assert_eq!(generation_id.len(), 26);
    assert_ne!(Ulid::from_string(generation_id).unwrap(), Ulid::nil());
}

#[tokio::test]
async fn exchange_lookup_returns_all_sources_in_deterministic_order() {
    let left = source_fixture(533, Duration::from_millis(50));
    let right = source_fixture(534, Duration::from_millis(50));
    publish(&left.cache, vec![row(533, "AAPL", "XNAS", "left")]);
    publish(&right.cache, vec![row(534, "AAPL", "XNAS", "right")]);
    let backend = backend(&[&right, &left], 8);

    let result = backend
        .get_by_exchange(ExchangeLookup {
            exchange: "XNAS".into(),
            code: "AAPL".into(),
            consistency: Consistency::CachePreferred,
        })
        .await
        .unwrap();

    assert_eq!(result.body.match_count, 2);
    assert_eq!(result.body.matches[0].data.source_id, 533);
    assert_eq!(result.body.matches[1].data.source_id, 534);
}

#[tokio::test]
async fn service_errors_map_to_typed_http_errors() {
    let source = source_fixture(533, Duration::from_millis(10));
    let backend = backend(&[&source], 8);

    let unsupported = backend
        .get_by_source(SourceLookup {
            source_id: 999,
            symbol: "AAPL".into(),
            consistency: Consistency::FreshRequired,
        })
        .await
        .unwrap_err();
    assert_eq!(unsupported.code, ApiErrorCode::UnsupportedSource);
    assert_eq!(unsupported.code.status().as_u16(), 422);

    let timeout = backend
        .get_by_source(SourceLookup {
            source_id: 533,
            symbol: "MISSING".into(),
            consistency: Consistency::FreshRequired,
        })
        .await
        .unwrap_err();
    assert_eq!(timeout.code, ApiErrorCode::UpstreamTimeout);
    assert!(timeout.retryable);
    assert_eq!(timeout.context.symbol.as_deref(), Some("MISSING"));
}

#[tokio::test]
async fn sync_job_runs_real_service_and_supports_idempotent_replay() {
    let source = source_fixture(533, Duration::from_millis(100));
    source.actions.lock().unwrap().push_back(Action::Whole(vec![
        row(533, "AAPL", "XNAS", "Apple"),
        row(533, "MSFT", "XNAS", "Microsoft"),
    ]));
    let backend = backend(&[&source], 8);

    let created = backend
        .create_sync_job(
            CreateSyncJobRequest { source_id: 533 },
            Some("same-request".into()),
        )
        .await
        .unwrap();
    assert!(!created.replayed);
    let replay = backend
        .create_sync_job(
            CreateSyncJobRequest { source_id: 533 },
            Some("same-request".into()),
        )
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.job.job_id, created.job.job_id);

    let completed = wait_for_terminal(&backend, &created.job.job_id).await;
    assert_eq!(completed.status, SyncJobStatus::Succeeded);
    assert_eq!(completed.received_records, 2);
    assert_eq!(completed.accepted_records, 2);
    let generation_id = completed.generation_id.as_deref().unwrap();
    assert_eq!(generation_id.len(), 26);
    assert_ne!(Ulid::from_string(generation_id).unwrap(), Ulid::nil());
    assert!(source.cache.get("AAPL").is_some());
}

#[tokio::test]
async fn sync_job_reports_precise_received_accepted_and_rejected_counts() {
    let source = source_fixture(533, Duration::from_millis(100));
    let mut partial = row(533, "MSFT", "XNAS", "Microsoft");
    partial.tokens.push(OwnedToken {
        number: 435,
        value: OwnedTokenValue::String("INVALID".to_owned()),
    });
    source.actions.lock().unwrap().push_back(Action::Whole(vec![
        row(533, "AAPL", "XNAS", "Apple"),
        row(533, "", "XNAS", "Invalid"),
        partial,
    ]));
    let backend = backend(&[&source], 8);

    let created = backend
        .create_sync_job(CreateSyncJobRequest { source_id: 533 }, None)
        .await
        .unwrap();
    let completed = wait_for_terminal(&backend, &created.job.job_id).await;

    assert_eq!(completed.status, SyncJobStatus::Succeeded);
    assert_eq!(completed.received_records, 3);
    assert_eq!(completed.accepted_records, 2);
    assert_eq!(completed.rejected_records, 1);
    assert_eq!(source.cache.snapshot().len(), 2);
}

#[tokio::test]
async fn manual_sync_marks_cache_ready_only_after_every_source_succeeds() {
    let left = source_fixture(533, Duration::from_millis(100));
    let right = source_fixture(534, Duration::from_millis(100));
    left.actions
        .lock()
        .unwrap()
        .push_back(Action::Whole(vec![row(533, "AAPL", "XNAS", "Apple")]));
    right
        .actions
        .lock()
        .unwrap()
        .push_back(Action::Whole(vec![row(534, "MSFT", "XNAS", "Microsoft")]));
    let readiness = Arc::new(ReadinessState::ready());
    readiness.set_cache(false);
    let backend = backend_with_readiness(&[&left, &right], 8, Arc::clone(&readiness));

    let first = backend
        .create_sync_job(CreateSyncJobRequest { source_id: 533 }, None)
        .await
        .unwrap();
    assert_eq!(
        wait_for_terminal(&backend, &first.job.job_id).await.status,
        SyncJobStatus::Succeeded
    );
    assert!(!backend.readiness().await.is_ready());

    let second = backend
        .create_sync_job(CreateSyncJobRequest { source_id: 534 }, None)
        .await
        .unwrap();
    assert_eq!(
        wait_for_terminal(&backend, &second.job.job_id).await.status,
        SyncJobStatus::Succeeded
    );
    assert!(backend.readiness().await.is_ready());
}

#[tokio::test]
async fn failed_manual_sync_does_not_mark_cache_ready() {
    let source = source_fixture(533, Duration::from_millis(10));
    publish(&source.cache, vec![row(533, "AAPL", "XNAS", "Apple")]);
    let readiness = Arc::new(ReadinessState::ready());
    readiness.set_cache(false);
    let backend = backend_with_readiness(&[&source], 8, Arc::clone(&readiness));

    let created = backend
        .create_sync_job(CreateSyncJobRequest { source_id: 533 }, None)
        .await
        .unwrap();
    assert_eq!(
        wait_for_terminal(&backend, &created.job.job_id)
            .await
            .status,
        SyncJobStatus::Failed
    );
    assert!(!backend.readiness().await.is_ready());
}

#[tokio::test]
async fn concurrent_idempotent_creates_return_one_job() {
    let source = source_fixture(533, Duration::from_millis(100));
    let backend = Arc::new(backend(&[&source], 8));
    let left_backend = Arc::clone(&backend);
    let right_backend = Arc::clone(&backend);

    let (left, right) = tokio::join!(
        left_backend.create_sync_job(
            CreateSyncJobRequest { source_id: 533 },
            Some("concurrent-key".into()),
        ),
        right_backend.create_sync_job(
            CreateSyncJobRequest { source_id: 533 },
            Some("concurrent-key".into()),
        )
    );
    let left = left.unwrap();
    let right = right.unwrap();

    assert_eq!(left.job.job_id, right.job.job_id);
    assert_ne!(left.replayed, right.replayed);
}

#[tokio::test]
async fn sync_jobs_enforce_source_and_idempotency_conflicts() {
    let left = source_fixture(533, Duration::from_millis(100));
    let right = source_fixture(534, Duration::from_millis(100));
    let backend = backend(&[&left, &right], 8);

    let first = backend
        .create_sync_job(
            CreateSyncJobRequest { source_id: 533 },
            Some("fixed-key".into()),
        )
        .await
        .unwrap();
    let active = backend
        .create_sync_job(CreateSyncJobRequest { source_id: 533 }, None)
        .await
        .unwrap_err();
    assert_eq!(active.code, ApiErrorCode::SyncAlreadyRunning);
    assert_eq!(
        active.context.active_job_id.as_deref(),
        Some(first.job.job_id.as_str())
    );

    let conflict = backend
        .create_sync_job(
            CreateSyncJobRequest { source_id: 534 },
            Some("fixed-key".into()),
        )
        .await
        .unwrap_err();
    assert_eq!(conflict.code, ApiErrorCode::IdempotencyConflict);

    let failed = wait_for_terminal(&backend, &first.job.job_id).await;
    assert_eq!(failed.status, SyncJobStatus::Failed);
    assert_eq!(failed.error.unwrap().code, ApiErrorCode::UpstreamTimeout);
}

#[tokio::test]
async fn readiness_is_local_and_requires_complete_source_caches() {
    let source = source_fixture(533, Duration::from_millis(50));
    let readiness = Arc::new(ReadinessState::ready());
    let services = BTreeMap::from([(533, Arc::clone(&source.service))]);
    let caches = BTreeMap::from([(533, Arc::clone(&source.cache))]);
    let backend = ContractBackend::new(
        services,
        caches,
        BackendConfig::default(),
        Arc::clone(&readiness),
    )
    .unwrap();

    assert!(!backend.readiness().await.is_ready());
    publish(&source.cache, vec![row(533, "AAPL", "XNAS", "Apple")]);
    assert!(backend.readiness().await.is_ready());
    readiness.set_cfapi_session(false);
    assert!(!backend.readiness().await.is_ready());
    readiness.set_cache(false);
    readiness.set_query_coordinator(false);
    readiness.set_sync_coordinator(false);
    let _not_ready = ReadinessState::not_ready();
}

async fn wait_for_terminal(
    backend: &ContractBackend,
    job_id: &str,
) -> contract_query_service::http::SyncJob {
    for _ in 0..100 {
        let job = backend.get_sync_job(job_id.to_owned()).await.unwrap();
        if matches!(
            job.status,
            SyncJobStatus::Succeeded | SyncJobStatus::Failed | SyncJobStatus::Cancelled
        ) {
            return job;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("job did not reach a terminal state");
}
