# HTTP And OpenAPI Contract

Status: decision for issue #4

## Conventions

- Base path: `/v1`.
- JSON property names: `snake_case`.
- Times: UTC RFC 3339 with an offset.
- Dates: `YYYY-MM-DD`.
- Decimal values: strings.
- Request and job IDs: ULID strings.
- Every response carries `x-request-id`.
- Symbols and codes are query parameters, not path segments, so reserved characters survive URL encoding and proxies.

OpenAPI tags are `Contracts`, `Synchronization`, and `Health`.

## Contract Lookup

### Canonical Source Lookup

```http
GET /v1/contracts/by-source?source_id=533&symbol=AAPL
```

Operation ID: `getContractBySource`.

This is the canonical lookup because `(source_id, symbol)` is the CFAPI Contract Key. `source_id` is a nonzero integer no greater than 65535. `symbol` is 1-128 UTF-8 bytes after URL decoding; NUL and control characters are rejected. Matching is exact and case-sensitive.

Optional `consistency` is `cache_preferred` (default) or `fresh_required`. `fresh_required` must not fall back to stale metadata after an upstream failure.

### Exchange Lookup

```http
GET /v1/contracts/by-exchange?exchange=XNGS&code=AAPL
```

Operation ID: `getContractByExchange`.

`exchange` is token 3241 listing/reference MIC, normalized to uppercase and validated as four ASCII alphanumeric characters. `code` follows the same rules as symbol.

The MIC index is an alias over canonical Contract Keys. No match returns 404. One or more matches return a collection in deterministic `(symbol bytes, source_id)` order; the service never guesses a preferred source. Each match carries its own freshness. Token 3240 `feed_mic` is not accepted as the `exchange` lookup key. `consistency` has the same semantics as source lookup.

### Success DTO

Source lookup returns the following HTTP 200 body. Exchange lookup wraps the same entries as `{ "matches": [...], "match_count": N }`.

```json
{
  "data": {
    "schema_version": 1,
    "source_id": 533,
    "exchange": "XNGS",
    "feed_mic": "XNAS",
    "code": "AAPL",
    "symbol": "AAPL",
    "name": "Apple Inc.",
    "category": null,
    "sector": null,
    "industry": null,
    "unit": "40",
    "unit_value": "40",
    "reference": null,
    "reference_observed_at": null,
    "currency": "USD",
    "instrument_type_code": 257,
    "instrument_type": "COMMON_STOCK",
    "tick_size": "0.0001 1 0.01",
    "tick_size_rules": [
      { "tick": "0.0001", "upper_bound": "1" },
      { "tick": "0.01", "upper_bound": null }
    ],
    "update_date": "2026-08-19",
    "metadata_observed_at": "2026-08-19T12:34:56Z"
  },
  "freshness": {
    "generation_id": "01K...",
    "metadata": "fresh",
    "metadata_age_seconds": 12,
    "fresh_until": "2026-08-19T12:39:56Z",
    "stale_since": null,
    "reference": "absent",
    "reference_age_seconds": null,
    "served_stale_due_to": null
  }
}
```

Freshness enums are `fresh`, `stale`, and `absent`. `metadata` cannot be absent on a successful Contract. A stale-if-error response remains 200, sets `metadata=stale`, and provides a stable reason code such as `upstream_unavailable` or `refresh_timeout`. Exact TTL and stale policy belong to issue #6.

Successful lookup responses include a content-derived `ETag`, `Last-Modified` from metadata observation, and `Cache-Control: private, no-cache`. `If-None-Match` may return 304. ETag input includes schema version, generation/content, and reference version; it never includes request time.

Cache miss may trigger one exact QueryXref request. Its configurable application timeout maps to 504 only when no usable stale Contract exists. A late upstream success after timeout cannot alter that HTTP response.

## Source Synchronization Jobs

### Create

```http
POST /v1/contract-sync-jobs
Idempotency-Key: 01K...
Content-Type: application/json

{ "source_id": 533 }
```

Operation ID: `createContractSyncJob`.

The body is limited to 8 KiB. The `Idempotency-Key` header is optional, 1-128 visible ASCII characters. Repeating the same key and equivalent body during job retention returns the original response. Reusing a key with a different body returns 409 `idempotency_conflict`.

An idempotent replay adds `Idempotency-Replayed: true`.

A new job returns 202, a `Location: /v1/contract-sync-jobs/{job_id}` header, and `SyncJob`. Another active job for the same source returns 409 `sync_already_running` with its `active_job_id`. There is no force flag or cancellation endpoint in v1.

### Status

```http
GET /v1/contract-sync-jobs/{job_id}
```

Operation ID: `getContractSyncJob`.

```json
{
  "job_id": "01K...",
  "source_id": 533,
  "status": "succeeded",
  "submitted_at": "2026-08-19T12:00:00Z",
  "started_at": "2026-08-19T12:00:00Z",
  "completed_at": "2026-08-19T12:00:03Z",
  "received_records": 6234,
  "accepted_records": 6233,
  "rejected_records": 1,
  "generation_id": "01K...",
  "error": null
}
```

Job statuses are `queued`, `running`, `succeeded`, `failed`, and `cancelled`. `error`, when present, uses the same code/message shape as API errors without a request ID.

The job resource is a control-plane summary, not a second contract result store. Completed rows become visible through contract lookup after the generation commits. V1 has no job collection or result pagination endpoint. Terminal jobs are retained for 24 hours by default, configurable with an enforced upper bound; unknown and expired IDs return 404.

## Health

```text
GET /health/live   operationId=getLiveness
GET /health/ready  operationId=getReadiness
```

Liveness returns 200 while the process event loop is responsive. Readiness returns 200 only when the CFAPI session, cache, and coordinators can accept work; otherwise it returns 503. Both read local state and never query CFAPI.

```json
{
  "status": "ready",
  "cfapi_session": "ready",
  "cache": "ready",
  "query_coordinator": "ready",
  "sync_coordinator": "ready"
}
```

## Errors

```json
{
  "code": "contract_not_found",
  "message": "contract was not found",
  "request_id": "01K...",
  "retryable": false,
  "context": {
    "source_id": 533,
    "symbol": "UNKNOWN"
  }
}
```

`context` is a reusable typed object with optional documented fields such as source ID, symbol, exchange, code, and active job ID; it is not a free-form map. Messages are safe for clients and contain no credentials or raw upstream payload.

| Status | Codes |
|---:|---|
| 400 | `invalid_request`, `invalid_source_id`, `invalid_symbol`, `invalid_exchange`, `invalid_code` |
| 403 | `permission_denied` |
| 404 | `contract_not_found`, `sync_job_not_found` |
| 409 | `sync_already_running`, `idempotency_conflict` |
| 413 | `request_too_large` |
| 422 | `unsupported_source` |
| 502 | `upstream_status`, `upstream_protocol_error` |
| 503 | `not_ready`, `upstream_unavailable`, `backpressure`, `shutting_down` |
| 504 | `upstream_timeout` |
| 500 | `internal_error`, `configuration_error` |

Framework 404, 405, panic, and malformed JSON use this same envelope. `Allow` remains present on 405 responses.

## OpenAPI Requirements

- Publish OpenAPI 3.1 JSON at `/api-doc/openapi.json` and Scalar at `/doc`.
- Every operation has a stable operation ID, tag, summary, parameter constraints, success schema, and all typed error responses.
- Reuse components for `Contract`, `ContractResponse`, `Freshness`, `TickSizeRule`, `SyncJob`, `HealthResponse`, and `ApiError`.
- Mark nullable fields explicitly and provide examples for fresh, stale, not-found, multi-match MIC, running job, and failed job.
- Do not expose secrets, CFAPI credentials, raw status payloads, or internal queue topology in schemas.

## Contract Tests

- Reserved symbol characters round-trip through query parsing, including `.`, space, slash, backslash, and percent.
- Same symbol on two sources remains distinct.
- Exchange lookup unique, missing, and multiple-match outcomes.
- Cache hit, miss, stale-if-error, timeout without stale data, and late result after timeout.
- Idempotent job replay, mismatched replay, same-source conflict, terminal status, and expired job.
- Every error status/code/envelope and framework 404/405.
- OpenAPI snapshot and generated-client compilation smoke test.
- Request ID preservation/generation, Location header, 8 KiB body limit, and readiness 503.
