#[path = "../src/service.rs"]
mod service;

use contract_query_service::{
    cache::{
        CacheLimits, Consistency, FreshnessDecision, FreshnessPolicy, RefreshFailure, SourceCache,
    },
    query::{CallbackClassification, PendingRegistry, QueryError, QueryTag, RegistryLimits},
    ContractKey, OwnedQueryXrefRow, OwnedToken, OwnedTokenValue,
};
use service::{
    CfapiCommandBus, CfapiCommandOwner, ContractService, OwnerExecuteError, OwnerQuery,
    ServiceConfig,
};
use std::{
    collections::VecDeque,
    num::NonZeroI64,
    sync::{
        atomic::{AtomicI64, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use time::OffsetDateTime;

enum Action {
    Exact(OwnedQueryXrefRow),
    DelayedExact(OwnedQueryXrefRow, Duration),
    Whole(Vec<OwnedQueryXrefRow>),
    WholeFailure(Vec<OwnedQueryXrefRow>),
    QueueFull,
    QueueFullWrongTag,
    NoCallback,
}

struct FakeOwner {
    registry: Arc<PendingRegistry>,
    actions: Arc<Mutex<VecDeque<Action>>>,
    prepares: Arc<AtomicUsize>,
    sends: Arc<AtomicUsize>,
    next_tag: AtomicI64,
    last_tag: Arc<Mutex<Option<QueryTag>>>,
}

impl CfapiCommandOwner for FakeOwner {
    fn execute(
        &mut self,
        query: &OwnerQuery,
        bind: &mut dyn FnMut(QueryTag) -> Result<(), QueryError>,
    ) -> Result<QueryTag, OwnerExecuteError> {
        match query {
            OwnerQuery::Exact { source_id, symbol } => {
                assert!(matches!(*source_id, 533 | 534));
                assert!(!symbol.is_empty());
            }
            OwnerQuery::WholeSource { source_id } => {
                assert!(matches!(*source_id, 533 | 534));
            }
        }
        self.prepares.fetch_add(1, Ordering::Relaxed);
        let tag = NonZeroI64::new(self.next_tag.fetch_add(1, Ordering::Relaxed)).unwrap();
        *self.last_tag.lock().unwrap() = Some(tag);
        let action = self.actions.lock().unwrap().pop_front().unwrap();
        bind(tag)?;
        self.sends.fetch_add(1, Ordering::Relaxed);
        match action {
            Action::Exact(row) => {
                self.registry.on_image_complete(tag, Some(row));
                Ok(tag)
            }
            Action::DelayedExact(row, delay) => {
                let registry = Arc::clone(&self.registry);
                std::thread::spawn(move || {
                    std::thread::sleep(delay);
                    registry.on_image_complete(tag, Some(row));
                });
                Ok(tag)
            }
            Action::Whole(rows) => {
                for row in rows {
                    self.registry.on_image_part(tag, row);
                }
                self.registry.on_image_complete(tag, None);
                Ok(tag)
            }
            Action::WholeFailure(rows) => {
                for row in rows {
                    self.registry.on_image_part(tag, row);
                }
                self.registry.on_status(tag, -12, "denied");
                Ok(tag)
            }
            Action::QueueFull => Err(OwnerExecuteError::SendQueueFull { tag }),
            Action::QueueFullWrongTag => Err(OwnerExecuteError::SendQueueFull {
                tag: NonZeroI64::new(tag.get() + 100).unwrap(),
            }),
            Action::NoCallback => Ok(tag),
        }
    }
}

struct Fixture {
    service: Arc<ContractService>,
    cache: Arc<SourceCache>,
    registry: Arc<PendingRegistry>,
    prepares: Arc<AtomicUsize>,
    sends: Arc<AtomicUsize>,
    last_tag: Arc<Mutex<Option<QueryTag>>>,
}

fn fixture(actions: Vec<Action>, timeout: Duration) -> Fixture {
    fixture_with_limits(actions, timeout, CacheLimits::default())
}

fn fixture_with_limits(
    actions: Vec<Action>,
    timeout: Duration,
    cache_limits: CacheLimits,
) -> Fixture {
    let cache = Arc::new(SourceCache::new(533, cache_limits));
    let registry = Arc::new(PendingRegistry::new(RegistryLimits::default()));
    let prepares = Arc::new(AtomicUsize::new(0));
    let sends = Arc::new(AtomicUsize::new(0));
    let last_tag = Arc::new(Mutex::new(None));
    let owner = FakeOwner {
        registry: Arc::clone(&registry),
        actions: Arc::new(Mutex::new(actions.into())),
        prepares: Arc::clone(&prepares),
        sends: Arc::clone(&sends),
        next_tag: AtomicI64::new(1),
        last_tag: Arc::clone(&last_tag),
    };
    let service = Arc::new(ContractService::new(
        Arc::clone(&cache),
        Arc::clone(&registry),
        owner,
        ServiceConfig {
            freshness: FreshnessPolicy {
                fresh_for: 10,
                max_stale_for: 100,
            },
            query_timeout: timeout,
            negative_ttl: 30,
            command_capacity: 4,
        },
    ));
    Fixture {
        service,
        cache,
        registry,
        prepares,
        sends,
        last_tag,
    }
}

fn row(symbol: &str, observed_at: i64) -> OwnedQueryXrefRow {
    row_for_source(533, symbol, observed_at)
}

fn row_for_source(source_id: u16, symbol: &str, observed_at: i64) -> OwnedQueryXrefRow {
    OwnedQueryXrefRow {
        source_id,
        symbol: symbol.to_owned(),
        tokens: vec![
            OwnedToken {
                number: 5,
                value: OwnedTokenValue::String(symbol.to_owned()),
            },
            OwnedToken {
                number: 3241,
                value: OwnedTokenValue::String("XNAS".to_owned()),
            },
        ],
        observed_at: OffsetDateTime::from_unix_timestamp(observed_at).unwrap(),
    }
}

#[tokio::test]
async fn two_sources_share_one_command_bus_registry_and_request_sequence() {
    let registry = Arc::new(PendingRegistry::new(RegistryLimits::default()));
    let prepares = Arc::new(AtomicUsize::new(0));
    let sends = Arc::new(AtomicUsize::new(0));
    let last_tag = Arc::new(Mutex::new(None));
    let actions = Arc::new(Mutex::new(
        vec![
            Action::DelayedExact(row_for_source(533, "AAPL", 100), Duration::from_millis(30)),
            Action::Exact(row_for_source(534, "MSFT", 100)),
        ]
        .into(),
    ));
    let owner = FakeOwner {
        registry: Arc::clone(&registry),
        actions,
        prepares: Arc::clone(&prepares),
        sends: Arc::clone(&sends),
        next_tag: AtomicI64::new(1),
        last_tag,
    };
    let bus = CfapiCommandBus::start(owner, Arc::clone(&registry), 4);
    let config = ServiceConfig {
        freshness: FreshnessPolicy {
            fresh_for: 10,
            max_stale_for: 100,
        },
        query_timeout: Duration::from_secs(1),
        negative_ttl: 30,
        command_capacity: 99,
    };
    let cache_533 = Arc::new(SourceCache::new(533, CacheLimits::default()));
    let cache_534 = Arc::new(SourceCache::new(534, CacheLimits::default()));
    let service_533 =
        ContractService::with_command_bus(Arc::clone(&cache_533), bus.clone(), config.clone());
    let service_534 = ContractService::with_command_bus(Arc::clone(&cache_534), bus, config);

    assert!(Arc::ptr_eq(service_533.registry(), service_534.registry()));
    assert!(Arc::ptr_eq(service_533.registry(), &registry));

    let now = OffsetDateTime::from_unix_timestamp(100).unwrap();
    let (left, right) = tokio::join!(
        service_533.lookup_exact(533, "AAPL", Consistency::FreshRequired, now),
        service_534.lookup_exact(534, "MSFT", Consistency::FreshRequired, now),
    );

    assert_eq!(left.unwrap().contract.metadata.key.symbol, "AAPL");
    assert_eq!(right.unwrap().contract.metadata.key.symbol, "MSFT");
    assert_eq!(prepares.load(Ordering::Relaxed), 2);
    assert_eq!(sends.load(Ordering::Relaxed), 2);
    assert_eq!(registry.pending_count(), 0);
    assert!(cache_533.get("AAPL").is_some());
    assert!(cache_533.get("MSFT").is_none());
    assert!(cache_534.get("MSFT").is_some());
    assert!(cache_534.get("AAPL").is_none());
}

fn seed(cache: &SourceCache, value: OwnedQueryXrefRow) {
    let parsed = contract_query_service::convert_query_xref_row(value, None).unwrap();
    cache.exact_upsert(Arc::new(parsed.contract)).unwrap();
}

#[tokio::test]
async fn fresh_exact_lookup_does_not_reach_cfapi_owner() {
    let fixture = fixture(Vec::new(), Duration::from_millis(100));
    seed(&fixture.cache, row("AAPL", 100));

    let result = fixture
        .service
        .lookup_exact(
            533,
            "AAPL",
            Consistency::CachePreferred,
            OffsetDateTime::from_unix_timestamp(105).unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(result.contract.metadata.key.symbol, "AAPL");
    assert_eq!(result.freshness, FreshnessDecision::Fresh);
    assert_eq!(fixture.prepares.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn exact_callback_may_finish_before_send_returns() {
    let fixture = fixture(
        vec![Action::Exact(row("MSFT", 100))],
        Duration::from_secs(1),
    );

    let result = fixture
        .service
        .lookup_exact(
            533,
            "MSFT",
            Consistency::FreshRequired,
            OffsetDateTime::from_unix_timestamp(100).unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(result.contract.metadata.key.symbol, "MSFT");
    assert_eq!(fixture.registry.pending_count(), 0);
    assert_eq!(fixture.sends.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn bind_failure_drops_prepared_request_without_sending() {
    let fixture = fixture(vec![Action::NoCallback], Duration::from_secs(1));
    let colliding_tag = NonZeroI64::new(1).unwrap();
    let existing = fixture.registry.register_exact(999, 533).unwrap();
    fixture.registry.bind(999, colliding_tag).unwrap();
    assert!(fixture.registry.cancel(999));
    drop(existing);

    let error = fixture
        .service
        .lookup_exact(
            533,
            "IBM",
            Consistency::FreshRequired,
            OffsetDateTime::from_unix_timestamp(100).unwrap(),
        )
        .await
        .unwrap_err();

    assert_eq!(
        error,
        service::ServiceError::Query(QueryError::TagCollision)
    );
    assert_eq!(fixture.prepares.load(Ordering::Relaxed), 1);
    assert_eq!(fixture.sends.load(Ordering::Relaxed), 0);
    assert_eq!(fixture.registry.pending_count(), 0);
}

#[tokio::test]
async fn concurrent_exact_lookups_share_one_request() {
    let fixture = fixture(
        vec![Action::DelayedExact(
            row("NVDA", 100),
            Duration::from_millis(20),
        )],
        Duration::from_secs(1),
    );
    let now = OffsetDateTime::from_unix_timestamp(100).unwrap();
    let left = fixture
        .service
        .lookup_exact(533, "NVDA", Consistency::FreshRequired, now);
    let right = fixture
        .service
        .lookup_exact(533, "NVDA", Consistency::FreshRequired, now);
    let (left, right) = tokio::join!(left, right);

    assert_eq!(left.unwrap().contract.metadata.key.symbol, "NVDA");
    assert_eq!(right.unwrap().contract.metadata.key.symbol, "NVDA");
    assert_eq!(fixture.prepares.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn cancelling_leader_releases_pending_request_and_singleflight_key() {
    let fixture = fixture(
        vec![
            Action::DelayedExact(row("ORCL", 100), Duration::from_millis(100)),
            Action::Exact(row("ORCL", 101)),
        ],
        Duration::from_secs(1),
    );
    let first_service = Arc::clone(&fixture.service);
    let first = tokio::spawn(async move {
        first_service
            .lookup_exact(
                533,
                "ORCL",
                Consistency::FreshRequired,
                OffsetDateTime::from_unix_timestamp(100).unwrap(),
            )
            .await
    });
    while fixture.prepares.load(Ordering::Relaxed) == 0 {
        tokio::task::yield_now().await;
    }
    first.abort();
    assert!(first.await.unwrap_err().is_cancelled());

    let retry = fixture
        .service
        .lookup_exact(
            533,
            "ORCL",
            Consistency::FreshRequired,
            OffsetDateTime::from_unix_timestamp(101).unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(retry.contract.metadata.key.symbol, "ORCL");
    assert_eq!(fixture.prepares.load(Ordering::Relaxed), 2);
    assert_eq!(fixture.registry.pending_count(), 0);
}

#[tokio::test]
async fn send_queue_full_rolls_back_and_late_callback_is_classified() {
    let fixture = fixture(vec![Action::QueueFull], Duration::from_secs(1));

    let error = fixture
        .service
        .lookup_exact(
            533,
            "AMD",
            Consistency::FreshRequired,
            OffsetDateTime::from_unix_timestamp(100).unwrap(),
        )
        .await
        .unwrap_err();
    assert_eq!(
        error,
        service::ServiceError::Query(QueryError::SendQueueFull)
    );
    assert_eq!(fixture.registry.pending_count(), 0);

    let tag = fixture.last_tag.lock().unwrap().unwrap();
    assert_eq!(
        service::classify_late_callback(&fixture.registry, tag, Some(row("AMD", 100))),
        CallbackClassification::Late
    );
}

#[tokio::test]
async fn send_queue_full_never_rolls_back_a_different_tag() {
    let fixture = fixture(vec![Action::QueueFullWrongTag], Duration::from_secs(1));

    let error = fixture
        .service
        .lookup_exact(
            533,
            "INTC",
            Consistency::FreshRequired,
            OffsetDateTime::from_unix_timestamp(100).unwrap(),
        )
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        service::ServiceError::Query(QueryError::ProtocolViolation(_))
    ));
    assert_eq!(fixture.registry.pending_count(), 0);
    let bound_tag = fixture.last_tag.lock().unwrap().unwrap();
    assert_eq!(
        fixture
            .registry
            .on_image_complete(bound_tag, Some(row("INTC", 100))),
        CallbackClassification::Late
    );
}

#[tokio::test]
async fn timeout_releases_request_and_quarantines_tag() {
    let fixture = fixture(vec![Action::NoCallback], Duration::from_millis(20));

    let error = fixture
        .service
        .lookup_exact(
            533,
            "META",
            Consistency::FreshRequired,
            OffsetDateTime::from_unix_timestamp(100).unwrap(),
        )
        .await
        .unwrap_err();
    assert_eq!(error, service::ServiceError::Query(QueryError::Timeout));
    assert_eq!(fixture.registry.pending_count(), 0);
    let tag = fixture.last_tag.lock().unwrap().unwrap();
    assert_eq!(
        service::classify_late_callback(&fixture.registry, tag, Some(row("META", 100))),
        CallbackClassification::Late
    );
}

#[tokio::test]
async fn cache_preferred_returns_bounded_stale_data_after_retryable_failure() {
    let fixture = fixture(vec![Action::QueueFull], Duration::from_secs(1));
    seed(&fixture.cache, row("TSLA", 100));

    let result = fixture
        .service
        .lookup_exact(
            533,
            "TSLA",
            Consistency::CachePreferred,
            OffsetDateTime::from_unix_timestamp(120).unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        result.freshness,
        FreshnessDecision::Stale(RefreshFailure::Backpressure)
    );
}

#[tokio::test]
async fn whole_source_commits_only_after_complete() {
    let fixture = fixture(
        vec![Action::Whole(vec![row("AAPL", 100), row("MSFT", 100)])],
        Duration::from_secs(1),
    );

    let outcome = fixture.service.sync_source(533).await.unwrap();

    assert!(outcome.generation.is_complete());
    assert_eq!(fixture.service.registry().pending_count(), 0);
    assert_eq!(outcome.generation.len(), 2);
    assert_eq!(outcome.received_records, 2);
    assert_eq!(outcome.accepted_records, 2);
    assert_eq!(outcome.rejected_records, 0);
    assert!(fixture.cache.get("AAPL").is_some());
    assert!(fixture.cache.get("MSFT").is_some());
}

#[tokio::test]
async fn whole_source_rejects_bad_rows_and_accepts_validation_issues() {
    let mut partial = row("MSFT", 100);
    partial.tokens.push(OwnedToken {
        number: 435,
        value: OwnedTokenValue::String("INVALID".to_owned()),
    });
    let fixture = fixture(
        vec![Action::Whole(vec![row("AAPL", 100), row("", 100), partial])],
        Duration::from_secs(1),
    );

    let outcome = fixture.service.sync_source(533).await.unwrap();

    assert_eq!(outcome.received_records, 3);
    assert_eq!(outcome.accepted_records, 2);
    assert_eq!(outcome.rejected_records, 1);
    assert_eq!(outcome.generation.len(), 2);
    assert!(fixture.cache.get("AAPL").is_some());
    assert!(fixture.cache.get("MSFT").is_some());
}

#[tokio::test]
async fn staging_limit_is_fatal_and_preserves_the_previous_generation() {
    let fixture = fixture_with_limits(
        vec![Action::Whole(vec![row("AAPL", 100), row("MSFT", 100)])],
        Duration::from_secs(1),
        CacheLimits {
            max_records: 1,
            ..CacheLimits::default()
        },
    );
    let before = fixture.cache.snapshot();

    let error = fixture.service.sync_source(533).await.unwrap_err();

    assert!(matches!(error, service::ServiceError::Staging(_)));
    let after = fixture.cache.snapshot();
    assert_eq!(after.id(), before.id());
    assert!(!after.is_complete());
    assert!(fixture.cache.get("AAPL").is_none());
}

#[tokio::test]
async fn staging_byte_limit_is_fatal() {
    let fixture = fixture_with_limits(
        vec![Action::Whole(vec![row("AAPL", 100)])],
        Duration::from_secs(1),
        CacheLimits {
            max_generation_bytes: 1,
            ..CacheLimits::default()
        },
    );

    let error = fixture.service.sync_source(533).await.unwrap_err();

    assert!(matches!(error, service::ServiceError::Staging(_)));
    assert!(!fixture.cache.snapshot().is_complete());
}

#[tokio::test]
async fn wrong_source_row_is_fatal_instead_of_rejected() {
    let fixture = fixture(
        vec![Action::Whole(vec![
            row("AAPL", 100),
            row_for_source(534, "MSFT", 100),
        ])],
        Duration::from_secs(1),
    );

    let error = fixture.service.sync_source(533).await.unwrap_err();

    assert!(matches!(error, service::ServiceError::Staging(_)));
    assert!(!fixture.cache.snapshot().is_complete());
    assert!(fixture.cache.get("AAPL").is_none());
}

#[tokio::test]
async fn failed_whole_source_does_not_publish_partial_generation() {
    let fixture = fixture(
        vec![Action::WholeFailure(vec![row("AAPL", 100)])],
        Duration::from_secs(1),
    );
    let before = fixture.cache.snapshot();

    let error = fixture.service.sync_source(533).await.unwrap_err();

    assert_eq!(
        error,
        service::ServiceError::Query(QueryError::PermissionDenied)
    );
    let after = fixture.cache.snapshot();
    assert_eq!(after.id(), before.id());
    assert!(!after.is_complete());
    assert!(fixture.cache.get("AAPL").is_none());
}

#[tokio::test]
async fn rejects_a_source_not_owned_by_the_cache() {
    let fixture = fixture(Vec::new(), Duration::from_secs(1));
    let error = fixture.service.sync_source(534).await.unwrap_err();
    assert_eq!(error, service::ServiceError::UnsupportedSource(534));
}

#[test]
fn owner_query_keeps_symbol_out_of_whole_source_requests() {
    let exact = OwnerQuery::Exact {
        source_id: 533,
        symbol: "AAPL".to_owned(),
    };
    let whole = OwnerQuery::WholeSource { source_id: 533 };
    assert!(matches!(exact, OwnerQuery::Exact { .. }));
    assert!(matches!(whole, OwnerQuery::WholeSource { .. }));
    let key = ContractKey {
        source_id: 533,
        symbol: "AAPL".to_owned(),
    };
    assert_eq!(key.symbol, "AAPL");
}

#[derive(Default)]
struct MockCfapi {
    sends: usize,
}

struct BorrowedPrepared<'a> {
    cfapi: &'a mut MockCfapi,
    tag: QueryTag,
}

impl BorrowedPrepared<'_> {
    fn send(self) {
        self.cfapi.sends += 1;
    }
}

#[derive(Default)]
struct BorrowingOwner {
    cfapi: MockCfapi,
}

impl CfapiCommandOwner for BorrowingOwner {
    fn execute(
        &mut self,
        _query: &OwnerQuery,
        bind: &mut dyn FnMut(QueryTag) -> Result<(), QueryError>,
    ) -> Result<QueryTag, OwnerExecuteError> {
        let prepared = BorrowedPrepared {
            cfapi: &mut self.cfapi,
            tag: NonZeroI64::new(77).unwrap(),
        };
        let tag = prepared.tag;
        bind(tag)?;
        prepared.send();
        Ok(tag)
    }
}

#[test]
fn owner_never_stores_prepared_state_across_trait_calls() {
    let mut owner = BorrowingOwner::default();
    let mut bound = None;
    let query = OwnerQuery::Exact {
        source_id: 533,
        symbol: "AAPL".to_owned(),
    };

    let returned = owner
        .execute(&query, &mut |tag| {
            bound = Some(tag);
            Ok(())
        })
        .unwrap();

    assert_eq!(bound, Some(returned));
    assert_eq!(owner.cfapi.sends, 1);
}
