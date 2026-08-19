# Optional Solace Contract Publication

Status: decision for issue #7

## Transport Seam

Solace is an optional projection of committed cache state, not query transport and not part of a CFAPI callback. Cache code emits a transport-neutral `ContractChange` only after an exact overlay or whole-source generation becomes visible. A publication coordinator serializes, queues, retries, and sends it through a `ContractPublisher` adapter.

Publication failure never rolls back cache state or changes a successful HTTP response.

## Topic

```text
IS/v2/CONTRACT/{exchange}/{code}
```

Snapshot control messages use `IS/v2/CONTRACT_SNAPSHOT/{source_id}`.

`exchange` is token 3241 listing/reference MIC. Records without exchange are queryable through HTTP but are not publishable and increment a validation metric. `source_id` remains in the payload because canonical identity is `(source_id, symbol)` and the same topic can receive more than one source projection.

Each topic segment is UTF-8 percent-encoded independently. Leave only RFC 3986 unreserved bytes (`A-Z a-z 0-9 - . _ ~`) unchanged; encode `%`, `/`, `*`, `>`, whitespace, and all other bytes using uppercase hex. This prevents symbols from changing Solace topic levels or becoming wildcards.

## MessagePack Envelope

Encode a named MessagePack map with `rmp_serde::to_vec_named`:

```rust
struct ContractPublication {
    schema_version: u16,       // 1
    event_type: EventType,     // upsert | delete
    mode: PublicationMode,     // delta | replay
    event_id: String,
    contract_version: String,  // canonical content hash
    publisher_epoch: String,
    source_sequence: u64,
    replay_id: Option<String>,
    generation_id: String,
    published_at: String,      // RFC 3339 UTC
    source_id: u16,
    exchange: String,
    code: String,
    contract: Option<ContractDto>,
}
```

Upsert contains the versioned Contract DTO. Delete has `contract=nil` but retains identity and the last known exchange/code. Decimal fields remain strings. Set Solace user property `ct=msgpack`.

`event_id` is deterministic from schema version, event type, canonical key, and contract version for delta, and from replay ID, key, and content hash for replay. Retries reuse identical topic and bytes. Consumers deduplicate by event ID and apply only the newest source sequence in a publisher epoch.

## Change Detection

- Exact-query Upsert publishes after its overlay is committed.
- Confirmed exact not-found publishes Delete after its delete overlay is committed.
- Whole-source commit diffs canonical content hashes against the previous visible state and emits only added, changed, and deleted Contracts.
- Observation-time-only changes do not publish.
- Reference value changes publish Upsert; a newer observation timestamp with the same reference value does not.
- GICS enrichment changes publish Upsert.

Normal scheduled whole sync is delta-only. Startup, reconnect, queue gap, and an optional jittered anti-entropy interval replay only the latest committed generation. Replay emits `snapshot_begin`, all source rows with one replay ID, then `snapshot_end`; control messages contain source, publisher epoch, generation ID, expected count, and manifest BLAKE3. Consumers replace source state only after count and manifest validate. Interrupted replay is discarded. The replay operation is internal and is not added to public HTTP v1.

## Ordering And Concurrency

Default to one publisher connection because contract changes are low frequency and a full 533/534 replay is only about 14,000 messages in the current environment. Configuration may allow 1-4 publishers. With more than one, route an entire source to one publisher by stable source hash so delta, snapshot markers, and replay rows retain source order.

The bounded command queue defaults to 1,024. Commands carry `Arc<Generation>`, one key mutation, or replay intent rather than prebuilding thousands of frames. Queueing occurs after cache commit on application workers, never on CFAPI callback threads. Serialization also happens outside callbacks.

## Failure And Gap Recovery

The rsolace adapter runs on its publisher OS thread. On disconnect or retryable send failure it retries the same encoded event with jittered exponential backoff from 100 ms to 30 seconds. Nonretryable serialization/validation errors go to a dead-letter metric and mark the source publication state as gapped.

V1 has no durable outbox, so delivery is at-least-once with eventual convergence, not exactly-once. Queue overflow, any non-OK send, rejected message, disconnect, or process restart marks affected sources dirty. After reconnect, the coordinator performs a rate-limited validated snapshot replay before resuming delta publication. Retry preserves identical topic, payload, event ID, epoch, and sequence.

If a newer event for the same key is queued before an unsent older Upsert, the coordinator may coalesce to the newest state. It must never coalesce away a Delete followed by a later Upsert or reverse their order. A sequence gap forces the consumer to wait for the next complete snapshot.

## Configuration And Readiness

`SOLACE_CONTRACT_ENABLED=false` installs a Noop publisher and creates no Solace client, queue, serialization, or retry work. Credentials have no defaults and are required only when enabled.

Solace remains optional in v1: unavailable publication reports `degraded` but does not make contract-query readiness fail. There is no required mode. Health output exposes only enabled/connected/gapped state, never host or credentials.

Use the existing `rsolace` v0.3.13 dependency and explicit session properties, including compression level required by the selected port. Do not reuse cfvhub's dotenv defaults or logging-only error handling.

## Metrics And Tests

Metrics include changes detected by type/source, queue messages/bytes, serialization errors, send success/failure/retry, reconnect duration, coalescing, replay rows/duration, gap transitions, nonpublishable missing-exchange rows, publisher partition counts, and end-to-end commit-to-send latency. Symbol and code are never metric labels.

Tests cover topic encoding for `/`, `%`, `*`, `>`, spaces, plus, and Unicode; named MessagePack golden payloads; deterministic event IDs; unchanged generation suppression; add/change/delete diff; reference value versus timestamp-only changes; exact overlay publication after visibility; per-key order with 1-4 publishers; disconnect/retry with identical bytes; queue overflow gap marking and replay; disabled mode performing no work; optional versus required readiness; and cache/HTTP success during Solace failure.
