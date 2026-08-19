# CFAPI Query Correlation And Completion

Status: decision for issue #5

## Primary Constraint

CFAPI 1.7 `Session.h` explicitly states that a `MessageEvent` may arrive before `Session::send()` returns. `Request::generateTag()` exists so the caller can know the tag first. The official sample performs `generateTag -> register under mutex -> send`. Register-after-send is therefore invalid.

The callback-owned `MessageEvent` becomes invalid when the callback returns. Required tokens must be copied into owned Rust data inside the callback.

## Driver Interface

```rust
trait CfapiDriver {
    type Prepared: PreparedRequest;
    fn prepare_query_xref(
        &mut self,
        request: QueryXrefRequest,
    ) -> Result<Self::Prepared, DriverError>;
}

trait PreparedRequest {
    fn tag(&self) -> NonZeroI64;
    fn send(self) -> Result<NonZeroI64, DriverError>;
}
```

The C++ prepared-request adapter creates and fills a Request, calls `generateTag()`, and frees the Request through RAII. `send()` must return the prepared tag. Return value 0 is `SendQueueFull`; any other tag mismatch is a protocol invariant failure.

Keep each CFAPI instance on one owner OS thread and operate it through a bounded command channel. Do not add `unsafe Send` to `CFAPI`, `Rc<RefCell<_>>`, or C++ pointers.

## Send Sequence

1. Acquire global and per-source concurrency permits.
2. Allocate `RequestId`, response channels, and cancellation guard.
3. Register the logical request in `by_request`.
4. On the owner thread, prepare QueryXref and obtain a nonzero tag.
5. Bind `(RequestId, QueryTag)` in the pending registry.
6. Call send. A callback may complete the request before send returns.
7. If send returns the tag, accept success even if pending has already become terminal.
8. If send returns 0, conditionally remove the same request/tag and complete it as `SendQueueFull`. Never overwrite a terminal callback result.

## Pending Registry

Use one `Arc<PendingRegistry>` with short `parking_lot::Mutex` critical sections:

```text
by_request: RequestId -> PendingEntry
by_tag: QueryTag -> Arc<PendingEntry>
tombstones: QueryTag -> TerminalReason + expiry
```

Tag 0 is invalid. Reject collision with an active tag or unexpired tombstone before send. Tombstones distinguish late, duplicate, and unknown callbacks; retain them for at least `max_query_timeout + 60s`, bounded initially to 65,536 entries.

Lookup or terminal ownership occurs under the registry lock. Parsing and channel sends occur after releasing it.

## State Machine

```text
Queued -> Bound(tag) -> Sent -> Terminal(reason)
                    \--------> Terminal(reason)
```

The direct `Bound -> Terminal` path is required because callbacks may precede send return.

- `IMAGE_PART`: copy and send one `OwnedContractRow`; do not complete.
- `IMAGE_COMPLETE` with a nonempty symbol: first send the final row, then complete.
- `IMAGE_COMPLETE` with an empty symbol: complete without a row.
- `STATUS`: terminal status error.
- status 14: `NotFound`; status -12: `PermissionDenied`.
- `UPDATE` or `REFRESH` for a QueryXref tag: `ProtocolViolation`.
- Only the callback that wins terminal ownership closes completion; duplicates become metrics.

## Channels And Callback Budget

Exact query uses a Tokio oneshot. Whole-source query uses a bounded Tokio mpsc item stream plus a completion oneshot.

The CFAPI callback may only read event metadata, copy selected fields into `OwnedContractRow`, perform registry operations, use nonblocking `try_send`, and update relaxed atomics. It must not mutate cache state, await, serialize JSON/MessagePack, publish Solace, build HTTP responses, or perform payload logging.

After terminal success, the whole-source aggregator drains the item channel before committing. This covers callback threads that already cloned a pending entry but have not yet sent their row.

## Limits And Backpressure

Initial configurable limits:

| Resource | Initial limit |
|---|---:|
| exact requests | 256 global, 32 per source |
| source sync | one per source, two globally |
| owner command queue | 1,024 |
| whole-source item queue | 4,096 |
| tombstones | 65,536 |

Each refresh generation also has `max_records` and `max_owned_bytes`. A full callback-to-aggregator queue ends the request as `ResponseBackpressure` and discards that generation; the callback never blocks.

## Cancellation, Session And Shutdown

CFAPI has no query cancellation operation. HTTP cancellation and timeout remove local waiters and create a tombstone; late callbacks are counted and ignored.

Session unavailable fails all pending requests. Source removal or source-specific recovery failure fails only that source. New requests are rejected while not ready. Session callback bridging must use an `Arc` thread-safe adapter rather than cross-thread `Rc<RefCell>`.

Whole-source rows enter a source-specific staging generation. Only successful matching terminal status, complete channel drain, and validation within limits permit one atomic generation swap. Timeout, cancellation, backpressure, disconnect, or shutdown discards staging and preserves the previous generation.

Shutdown first rejects new commands, resolves pending waiters as `ShuttingDown`, stops the CFAPI session, joins callback/owner work, and only then drops registries and handlers.

## Error Taxonomy

`LocalBackpressure`, `SendQueueFull`, `SessionUnavailable`, `Timeout`, `Cancelled`, `NotFound`, `PermissionDenied`, `CfapiStatus`, `ResponseBackpressure`, `GenerationLimit`, `ProtocolViolation`, `TagCollision`, and `ShuttingDown` remain distinct typed errors.

## Required Tests

The fake driver must synchronously trigger callback delivery from inside `send()` before returning. Also test send=0 rollback, exact terminal carrying data, whole-source final row, cancellation before and after bind, timeout/terminal race, duplicate/unknown/late tags, tag quarantine, item overflow, session disconnect, preservation of the old generation, and callback-safe shutdown.

