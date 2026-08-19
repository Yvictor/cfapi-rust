# Contract Cache And Refresh Policy

Status: decision for issue #6

## Structure

Each source owns an immutable published `Generation` behind `ArcSwap`, a short `mutation_gate`, exact-query overlays, bounded negative entries, and per-key singleflight. A generation contains canonical `(source_id, symbol)` rows plus `(exchange token 3241, code) -> Vec<ContractKey>` secondary index. Published generations are never mutated row by row.

```text
SourceCache
  published: ArcSwap<Generation>
  mutation_gate: Mutex<MutationState { revision, overlays, negatives }>
  exact_singleflight: ContractKey -> SharedQuery
```

Overlay entries are revisioned `Upsert(row)` or `Delete`. The MIC index always returns all candidates; MIC is uppercase-normalized while code and symbol remain opaque and case-sensitive.

## Freshness Defaults

| Value | Default | Configuration range |
|---|---:|---:|
| metadata positive TTL | 6h | 1m..24h |
| exact not-found TTL | 60s | 1s..5m |
| metadata maximum stale | 24h | 0..7d |
| reference fresh TTL | 5s | 1s..60s |
| reference maximum stale | 60s | 5s..10m |

TTL decisions use a monotonic clock; public observation times use UTC.

`cache_preferred` may serve stale metadata after session unavailable, timeout, local/send backpressure, retryable transport/status failure, or failed whole-source refresh. It must not serve stale for `fresh_required`, permission failure, unsupported source, confirmed not-found, protocol/validation failure, or data older than maximum stale.

Permission failure blocks that source's cached data immediately. Reference freshness is independent: stale reference is marked, and reference older than maximum stale becomes null without failing the Contract.

## Exact Lookup

Only one upstream exact query runs per Contract Key. A fresh complete generation proves an absent key is not found. Absence from a stale generation triggers an exact query.

Exact success creates an Upsert overlay. Confirmed exact not-found creates a Delete overlay and negative entry. Negative expiry permits a retry but must not resurrect a hidden old positive row. Timeout, permission, and backpressure never write negative cache. Cancelled or timed-out late callbacks never write cache.

## Whole-Source Commit Race

At sync start, capture `sync_start_revision` under `mutation_gate`. Exact results and whole commits linearize through the same gate.

1. Build bounded staging and receive a successful terminal event.
2. Drain the callback item channel completely.
3. Acquire `mutation_gate`.
4. Apply every overlay with `revision > sync_start_revision` to staging; Upsert wins and Delete removes the row.
5. Rebuild the secondary index and validate limits.
6. Assign a generation ID and atomically swap.
7. Clear merged positive overlays and retain required negative tombstones.

An exact result completed after sync started always wins over that staging data. If its callback acquires the gate after commit, it writes an overlay above the new generation. An exact overlay from before sync start may be replaced by the complete newer generation. Only one whole sync per source runs at once.

## Hashes And ETags

Precompute BLAKE3 over canonical fixed-order field encoding with explicit nulls and decimal strings; do not hash incidental JSON serialization. Exact ETag includes schema version, base generation ID, overlay revision, content hash, and reference version. MIC collection ETag additionally includes alias version and sorted candidate hashes. Request time is excluded.

## Limits

| Resource | Default |
|---|---:|
| records per source | 100,000 |
| generation bytes per source | 256 MiB |
| total cache bytes | 1 GiB |
| total staging bytes | 512 MiB |
| exact overlays per source | 10,000 |
| negative entries globally | 100,000 |

Memory accounting includes current, staging, retained generations, overlays, and indexes. Exceeding a staging limit aborts the generation and preserves current data. Complete source generations do not use record-level LRU. Reclaim expired negatives and retained generations first; reject new exact refresh with backpressure rather than silently dropping overlays.

V1 retains only current generation for serving because the HTTP contract has no list cursor. An old generation remains alive naturally while readers hold its `Arc`, then is reclaimed. No explicit historical generation store is required.

## Startup And Persistence

V1 is memory-only. Sources 533 and 534 contain roughly 14,000 records in the canned environment, so cold whole-source sync is simpler and safer than snapshot migration and corruption handling.

Startup synchronizes required sources in parallel. Readiness requires an established CFAPI session, one complete generation for every required source, open coordinators, and capacity below refusal thresholds. Exact-query success alone does not make the service ready.

Add durable snapshots only if measured cold-start time requires them; snapshots would need schema version, checksum, atomic replace, and credential isolation.

## Required Tests And Metrics

Tests must cover update/delete overlays before, during, and after commit; readers seeing only complete generations; MIC index consistency; 10,000-run randomized barrier stress plus Loom linearization; fake-clock TTL boundaries; stale permission blocking; memory-limit abort; canonical hash golden values; singleflight; and late callback rejection.

Metrics cover positive/negative/stale hits, misses, stale reason, singleflight joins, metadata/reference ages, staging rows/bytes/duration, overlays merged/deleted, memory by category, limit rejection, MIC candidate counts, ETag 304, and readiness transitions. Symbol is never a metric label.

