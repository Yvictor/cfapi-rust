use contract_query_service::cache::{
    decide_freshness, CacheLimits, Consistency, FreshnessDecision, FreshnessPolicy, RefreshFailure,
    SourceCache, StageError,
};
use contract_query_service::{ContractKey, ContractMetadata, ContractView, Mic};
use std::sync::Arc;
use time::macros::datetime;

fn contract(source_id: u16, symbol: &str, exchange: &str, name: &str) -> Arc<ContractView> {
    Arc::new(ContractView::from_metadata(ContractMetadata {
        key: ContractKey {
            source_id,
            symbol: symbol.to_owned(),
        },
        name: Some(name.to_owned()),
        feed_mic: None,
        exchange: Mic::parse(exchange),
        contract_size: None,
        currency: None,
        instrument_type: None,
        tick_size: None,
        metadata_observed_at: datetime!(2026-08-19 00:00 UTC),
    }))
}

fn name(row: &ContractView) -> &str {
    row.metadata.name.as_deref().unwrap()
}

#[test]
fn exact_overlay_before_sync_is_replaced_by_complete_generation() {
    let cache = SourceCache::new(533, CacheLimits::default());
    cache
        .exact_upsert(contract(533, "AAPL", "XNAS", "exact-before"))
        .unwrap();

    let mut sync = cache.begin_sync();
    sync.push(contract(533, "AAPL", "XNAS", "whole-newer"))
        .unwrap();
    cache.commit_sync(sync).unwrap();

    assert_eq!(name(&cache.get("AAPL").unwrap()), "whole-newer");
}

#[test]
fn exact_update_during_sync_wins_over_staging() {
    let cache = SourceCache::new(533, CacheLimits::default());
    let mut sync = cache.begin_sync();
    sync.push(contract(533, "AAPL", "XNAS", "staging")).unwrap();

    cache
        .exact_upsert(contract(533, "AAPL", "XNYS", "exact-during"))
        .unwrap();
    cache.commit_sync(sync).unwrap();

    let row = cache.get("AAPL").unwrap();
    assert_eq!(name(&row), "exact-during");
    assert_eq!(row.metadata.exchange.as_ref().unwrap().as_str(), "XNYS");
}

#[test]
fn exact_delete_during_sync_removes_staged_row() {
    let cache = SourceCache::new(533, CacheLimits::default());
    let mut sync = cache.begin_sync();
    sync.push(contract(533, "AAPL", "XNAS", "staging")).unwrap();

    cache.exact_delete("AAPL", 100).unwrap();
    cache.commit_sync(sync).unwrap();

    assert!(cache.get("AAPL").is_none());
    assert!(cache.is_negative("AAPL", 99));
    assert!(!cache.is_negative("AAPL", 100));
}

#[test]
fn exact_delete_before_sync_can_be_replaced_by_complete_generation() {
    let cache = SourceCache::new(533, CacheLimits::default());
    cache.exact_delete("AAPL", 100).unwrap();

    let mut sync = cache.begin_sync();
    sync.push(contract(533, "AAPL", "XNAS", "whole-newer"))
        .unwrap();
    cache.commit_sync(sync).unwrap();

    assert_eq!(name(&cache.get("AAPL").unwrap()), "whole-newer");
    assert!(!cache.is_negative("AAPL", 50));
}

#[test]
fn exact_update_after_commit_is_visible_over_published_generation() {
    let cache = SourceCache::new(533, CacheLimits::default());
    let mut sync = cache.begin_sync();
    sync.push(contract(533, "AAPL", "XNAS", "published"))
        .unwrap();
    cache.commit_sync(sync).unwrap();

    cache
        .exact_upsert(contract(533, "AAPL", "XNYS", "exact-after"))
        .unwrap();

    assert_eq!(name(&cache.get("AAPL").unwrap()), "exact-after");
}

#[test]
fn exact_delete_after_commit_hides_published_row() {
    let cache = SourceCache::new(533, CacheLimits::default());
    let mut sync = cache.begin_sync();
    sync.push(contract(533, "AAPL", "XNAS", "published"))
        .unwrap();
    cache.commit_sync(sync).unwrap();

    cache.exact_delete("AAPL", 100).unwrap();

    assert!(cache.get("AAPL").is_none());
    assert!(cache.by_exchange_code("XNAS", "AAPL").is_empty());
}

#[test]
fn snapshot_readers_see_a_complete_old_or_new_generation() {
    let cache = SourceCache::new(533, CacheLimits::default());
    let mut first = cache.begin_sync();
    first
        .push(contract(533, "AAPL", "XNAS", "old-aapl"))
        .unwrap();
    first
        .push(contract(533, "MSFT", "XNAS", "old-msft"))
        .unwrap();
    cache.commit_sync(first).unwrap();
    let old = cache.snapshot();

    let mut second = cache.begin_sync();
    second
        .push(contract(533, "NVDA", "XNAS", "new-nvda"))
        .unwrap();
    cache.commit_sync(second).unwrap();
    let new = cache.snapshot();

    assert_eq!(old.len(), 2);
    assert!(old.get("AAPL").is_some());
    assert!(old.get("NVDA").is_none());
    assert_eq!(new.len(), 1);
    assert!(new.get("AAPL").is_none());
    assert!(new.get("NVDA").is_some());
}

#[test]
fn exchange_index_tracks_overlay_exchange_changes_and_deletes() {
    let cache = SourceCache::new(533, CacheLimits::default());
    let mut sync = cache.begin_sync();
    sync.push(contract(533, "AAPL", "XNAS", "base")).unwrap();
    cache.commit_sync(sync).unwrap();

    cache
        .exact_upsert(contract(533, "AAPL", "XNYS", "moved"))
        .unwrap();
    assert!(cache.by_exchange_code("XNAS", "AAPL").is_empty());
    assert_eq!(cache.by_exchange_code("xnys", "AAPL").len(), 1);

    cache.exact_delete("AAPL", 50).unwrap();
    assert!(cache.by_exchange_code("XNYS", "AAPL").is_empty());
}

#[test]
fn exchange_lookup_returns_all_source_local_candidates() {
    let cache = SourceCache::new(533, CacheLimits::default());
    let mut sync = cache.begin_sync();
    sync.push(contract(533, "ABC", "XNAS", "first")).unwrap();
    sync.push(contract(533, "abc", "XNAS", "second")).unwrap();
    cache.commit_sync(sync).unwrap();

    assert_eq!(cache.by_exchange_code("XNAS", "ABC").len(), 1);
    assert_eq!(cache.by_exchange_code("XNAS", "abc").len(), 1);
}

#[test]
fn staging_limits_abort_without_replacing_published_generation() {
    let limits = CacheLimits {
        max_records: 1,
        max_generation_bytes: usize::MAX,
        max_overlays: 10,
    };
    let cache = SourceCache::new(533, limits);
    let mut first = cache.begin_sync();
    first
        .push(contract(533, "AAPL", "XNAS", "published"))
        .unwrap();
    cache.commit_sync(first).unwrap();

    let mut rejected = cache.begin_sync();
    rejected.push(contract(533, "MSFT", "XNAS", "one")).unwrap();
    assert_eq!(
        rejected.push(contract(533, "NVDA", "XNAS", "two")),
        Err(StageError::RecordLimit { limit: 1 })
    );

    assert_eq!(name(&cache.get("AAPL").unwrap()), "published");
    assert!(cache.get("MSFT").is_none());
}

#[test]
fn merged_overlay_limit_failure_preserves_published_generation() {
    let limits = CacheLimits {
        max_records: 1,
        max_generation_bytes: usize::MAX,
        max_overlays: 10,
    };
    let cache = SourceCache::new(533, limits);
    let mut first = cache.begin_sync();
    first
        .push(contract(533, "AAPL", "XNAS", "published"))
        .unwrap();
    cache.commit_sync(first).unwrap();

    let mut rejected = cache.begin_sync();
    rejected
        .push(contract(533, "MSFT", "XNAS", "staging"))
        .unwrap();
    cache
        .exact_upsert(contract(533, "NVDA", "XNAS", "overlay"))
        .unwrap();

    assert!(cache.commit_sync(rejected).is_err());
    let published = cache.snapshot();
    assert_eq!(published.len(), 1);
    assert_eq!(name(&published.get("AAPL").unwrap()), "published");
    assert!(published.get("MSFT").is_none());
}

#[test]
fn overlay_limit_rejects_new_keys_without_dropping_existing_overlay() {
    let limits = CacheLimits {
        max_records: 10,
        max_generation_bytes: usize::MAX,
        max_overlays: 1,
    };
    let cache = SourceCache::new(533, limits);

    cache
        .exact_upsert(contract(533, "AAPL", "XNAS", "kept"))
        .unwrap();
    assert!(cache
        .exact_upsert(contract(533, "MSFT", "XNAS", "rejected"))
        .is_err());

    assert_eq!(name(&cache.get("AAPL").unwrap()), "kept");
    assert!(cache.get("MSFT").is_none());
}

#[test]
fn freshness_policy_only_serves_allowed_stale_failures() {
    let policy = FreshnessPolicy {
        fresh_for: 10,
        max_stale_for: 30,
    };

    assert_eq!(
        decide_freshness(9, Consistency::CachePreferred, None, policy),
        FreshnessDecision::Fresh
    );
    assert_eq!(
        decide_freshness(
            20,
            Consistency::CachePreferred,
            Some(RefreshFailure::Timeout),
            policy,
        ),
        FreshnessDecision::Stale(RefreshFailure::Timeout)
    );
    assert_eq!(
        decide_freshness(
            20,
            Consistency::FreshRequired,
            Some(RefreshFailure::Timeout),
            policy,
        ),
        FreshnessDecision::Unavailable
    );
    assert_eq!(
        decide_freshness(
            5,
            Consistency::CachePreferred,
            Some(RefreshFailure::PermissionDenied),
            policy,
        ),
        FreshnessDecision::Unavailable
    );
    assert_eq!(
        decide_freshness(
            20,
            Consistency::CachePreferred,
            Some(RefreshFailure::PermissionDenied),
            policy,
        ),
        FreshnessDecision::Unavailable
    );
    assert_eq!(
        decide_freshness(
            31,
            Consistency::CachePreferred,
            Some(RefreshFailure::SessionUnavailable),
            policy,
        ),
        FreshnessDecision::Unavailable
    );
}

#[test]
fn singleflight_admits_one_query_per_key_until_finished() {
    let cache = SourceCache::new(533, CacheLimits::default());
    let key = ContractKey {
        source_id: 533,
        symbol: "AAPL".to_owned(),
    };

    assert!(cache.try_begin_exact(&key));
    assert!(!cache.try_begin_exact(&key));
    cache.finish_exact(&key);
    assert!(cache.try_begin_exact(&key));
}
