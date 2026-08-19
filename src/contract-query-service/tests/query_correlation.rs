use contract_query_service::{
    domain::OwnedQueryXrefRow,
    query::{
        CallbackClassification, PendingRegistry, QueryError, QueryTag, RegistryLimits,
        SendDisposition,
    },
};
use std::{num::NonZeroI64, sync::Arc, time::Duration};
use time::OffsetDateTime;

fn tag(value: i64) -> QueryTag {
    NonZeroI64::new(value).unwrap()
}

fn row(source_id: u16, symbol: &str) -> OwnedQueryXrefRow {
    OwnedQueryXrefRow {
        source_id,
        symbol: symbol.to_owned(),
        tokens: Vec::new(),
        observed_at: OffsetDateTime::UNIX_EPOCH,
    }
}

fn limits() -> RegistryLimits {
    RegistryLimits {
        max_exact: 4,
        max_exact_per_source: 2,
        max_whole_source: 2,
        item_queue_capacity: 2,
        max_records: 3,
        max_owned_bytes: 1_024,
        tombstone_capacity: 8,
        tombstone_ttl: Duration::from_secs(60),
    }
}

struct FakePrepared {
    tag: QueryTag,
    registry: Arc<PendingRegistry>,
    final_row: OwnedQueryXrefRow,
    result: i64,
}

impl FakePrepared {
    fn send(self) -> i64 {
        // CFAPI may synchronously invoke the callback before Session::send returns.
        self.registry
            .on_image_complete(self.tag, Some(self.final_row));
        self.result
    }
}

#[tokio::test]
async fn callback_can_complete_bound_request_before_send_returns() {
    let registry = Arc::new(PendingRegistry::new(limits()));
    let query = registry.register_exact(1, 533).unwrap();
    registry.bind(1, tag(11)).unwrap();
    let prepared = FakePrepared {
        tag: tag(11),
        registry: Arc::clone(&registry),
        final_row: row(533, "AAPL"),
        result: 11,
    };

    assert_eq!(prepared.send(), 11);
    assert_eq!(
        registry.mark_sent(1, tag(11)).unwrap(),
        SendDisposition::AlreadyTerminal
    );
    assert_eq!(query.receive().await.unwrap().symbol, "AAPL");
}

#[tokio::test]
async fn send_queue_full_rolls_back_only_matching_live_request() {
    let registry = PendingRegistry::new(limits());
    let query = registry.register_exact(1, 533).unwrap();
    registry.bind(1, tag(11)).unwrap();

    assert!(!registry.rollback_send_queue_full(1, tag(12)));
    assert!(registry.rollback_send_queue_full(1, tag(11)));
    assert!(!registry.rollback_send_queue_full(1, tag(11)));
    assert_eq!(query.receive().await, Err(QueryError::SendQueueFull));
}

#[tokio::test]
async fn normal_send_moves_bound_request_to_sent() {
    let registry = PendingRegistry::new(limits());
    let query = registry.register_exact(1, 533).unwrap();
    registry.bind(1, tag(11)).unwrap();

    assert_eq!(
        registry.mark_sent(1, tag(11)).unwrap(),
        SendDisposition::Sent
    );
    registry.on_image_complete(tag(11), Some(row(533, "AAPL")));
    assert_eq!(query.receive().await.unwrap().symbol, "AAPL");
}

#[tokio::test]
async fn exact_complete_requires_and_delivers_terminal_row() {
    let registry = PendingRegistry::new(limits());
    let query = registry.register_exact(1, 533).unwrap();
    registry.bind(1, tag(11)).unwrap();
    assert_eq!(
        registry.on_image_complete(tag(11), Some(row(533, "MSFT"))),
        CallbackClassification::Delivered
    );
    assert_eq!(query.receive().await.unwrap().symbol, "MSFT");

    let missing = registry.register_exact(2, 533).unwrap();
    registry.bind(2, tag(12)).unwrap();
    registry.on_image_complete(tag(12), None);
    assert!(matches!(
        missing.receive().await,
        Err(QueryError::ProtocolViolation(_))
    ));
}

#[tokio::test]
async fn whole_source_delivers_image_part_and_final_row_before_completion() {
    let registry = PendingRegistry::new(limits());
    let query = registry.register_whole_source(1, 533).unwrap();
    registry.bind(1, tag(11)).unwrap();
    registry.on_image_part(tag(11), row(533, "AAPL"));
    registry.on_image_complete(tag(11), Some(row(533, "MSFT")));

    let rows = query.drain().await.unwrap();
    assert_eq!(
        rows.iter()
            .map(|item| item.symbol.as_str())
            .collect::<Vec<_>>(),
        ["AAPL", "MSFT"]
    );
}

#[tokio::test]
async fn cancellation_before_and_after_bind_is_safe() {
    let registry = PendingRegistry::new(limits());
    let before = registry.register_exact(1, 533).unwrap();
    assert!(registry.cancel(1));
    assert_eq!(before.receive().await, Err(QueryError::Cancelled));

    let after = registry.register_exact(2, 533).unwrap();
    registry.bind(2, tag(12)).unwrap();
    assert!(registry.cancel(2));
    assert_eq!(after.receive().await, Err(QueryError::Cancelled));
    assert_eq!(
        registry.on_image_complete(tag(12), Some(row(533, "AAPL"))),
        CallbackClassification::Late
    );
}

#[tokio::test]
async fn timeout_and_terminal_race_has_one_winner() {
    for timeout_first in [true, false] {
        let registry = PendingRegistry::new(limits());
        let query = registry.register_exact(1, 533).unwrap();
        registry.bind(1, tag(11)).unwrap();
        if timeout_first {
            assert!(registry.timeout(1));
            assert_eq!(
                registry.on_image_complete(tag(11), Some(row(533, "AAPL"))),
                CallbackClassification::Late
            );
            assert_eq!(query.receive().await, Err(QueryError::Timeout));
        } else {
            registry.on_image_complete(tag(11), Some(row(533, "AAPL")));
            assert!(!registry.timeout(1));
            assert_eq!(query.receive().await.unwrap().symbol, "AAPL");
        }
    }
}

#[tokio::test]
async fn tombstones_classify_duplicates_late_and_unknown_and_quarantine_tags() {
    let registry = PendingRegistry::new(limits());
    let active = registry.register_exact(10, 534).unwrap();
    registry.bind(10, tag(10)).unwrap();
    let active_collision = registry.register_exact(20, 534).unwrap();
    assert_eq!(registry.bind(20, tag(10)), Err(QueryError::TagCollision));
    registry.cancel(20);
    assert_eq!(active_collision.receive().await, Err(QueryError::Cancelled));

    let complete = registry.register_exact(1, 533).unwrap();
    registry.bind(1, tag(11)).unwrap();
    registry.on_image_complete(tag(11), Some(row(533, "AAPL")));
    complete.receive().await.unwrap();
    assert_eq!(
        registry.on_image_complete(tag(11), Some(row(533, "AAPL"))),
        CallbackClassification::Duplicate
    );
    assert_eq!(
        registry.on_image_complete(tag(99), Some(row(533, "AAPL"))),
        CallbackClassification::Unknown
    );

    let collision = registry.register_exact(2, 533).unwrap();
    assert_eq!(registry.bind(2, tag(11)), Err(QueryError::TagCollision));
    assert!(registry.cancel(2));
    assert_eq!(collision.receive().await, Err(QueryError::Cancelled));

    let late = registry.register_exact(3, 533).unwrap();
    registry.bind(3, tag(13)).unwrap();
    registry.timeout(3);
    assert_eq!(
        registry.on_status(tag(13), 14, "not found"),
        CallbackClassification::Late
    );
    assert_eq!(late.receive().await, Err(QueryError::Timeout));
    registry.cancel(10);
    assert_eq!(active.receive().await, Err(QueryError::Cancelled));
}

#[tokio::test]
async fn whole_source_queue_overflow_never_blocks_callback() {
    let mut configured = limits();
    configured.item_queue_capacity = 1;
    let registry = PendingRegistry::new(configured);
    let mut query = registry.register_whole_source(1, 533).unwrap();
    registry.bind(1, tag(11)).unwrap();
    registry.on_image_part(tag(11), row(533, "AAPL"));
    registry.on_image_part(tag(11), row(533, "MSFT"));

    assert_eq!(query.items.recv().await.unwrap().symbol, "AAPL");
    assert_eq!(
        query.completion().await,
        Err(QueryError::ResponseBackpressure)
    );
}

#[tokio::test]
async fn generation_limit_fails_whole_source_without_blocking() {
    let mut configured = limits();
    configured.max_records = 1;
    let registry = PendingRegistry::new(configured);
    let query = registry.register_whole_source(1, 533).unwrap();
    registry.bind(1, tag(11)).unwrap();
    registry.on_image_part(tag(11), row(533, "AAPL"));
    registry.on_image_part(tag(11), row(533, "MSFT"));
    assert_eq!(query.drain().await, Err(QueryError::GenerationLimit));
}

#[tokio::test]
async fn status_mapping_source_failure_and_shutdown_are_typed() {
    let registry = PendingRegistry::new(limits());
    let not_found = registry.register_exact(1, 533).unwrap();
    registry.bind(1, tag(11)).unwrap();
    registry.on_status(tag(11), 14, "not found");
    assert_eq!(not_found.receive().await, Err(QueryError::NotFound));

    let disconnected = registry.register_exact(2, 534).unwrap();
    registry.bind(2, tag(12)).unwrap();
    assert_eq!(registry.fail_source(534, QueryError::SessionUnavailable), 1);
    assert_eq!(
        disconnected.receive().await,
        Err(QueryError::SessionUnavailable)
    );
    assert_eq!(
        registry.on_image_complete(tag(12), Some(row(534, "MSFT"))),
        CallbackClassification::Late
    );

    let shutting_down = registry.register_exact(3, 533).unwrap();
    assert_eq!(registry.shutdown(), 1);
    assert_eq!(shutting_down.receive().await, Err(QueryError::ShuttingDown));
    assert!(matches!(
        registry.register_exact(4, 533),
        Err(QueryError::ShuttingDown)
    ));
}

#[test]
fn concurrency_limits_are_enforced_by_kind_and_source() {
    let mut configured = limits();
    configured.max_exact = 2;
    configured.max_exact_per_source = 1;
    configured.max_whole_source = 1;
    let registry = PendingRegistry::new(configured);

    let _first = registry.register_exact(1, 533).unwrap();
    assert!(matches!(
        registry.register_exact(2, 533),
        Err(QueryError::LocalBackpressure)
    ));
    let _second = registry.register_exact(3, 534).unwrap();
    assert!(matches!(
        registry.register_exact(4, 535),
        Err(QueryError::LocalBackpressure)
    ));

    let _sync = registry.register_whole_source(5, 533).unwrap();
    assert!(matches!(
        registry.register_whole_source(6, 534),
        Err(QueryError::LocalBackpressure)
    ));
}
