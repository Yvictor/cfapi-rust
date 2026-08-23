# Contract Query Service Operations

## Architecture

The runtime uses one dedicated CFAPI owner thread for all mutable CFAPI request state. A shared,
bounded `CfapiCommandBus` serializes QueryXref requests onto that owner. Source 533 and 534 each
have an independent cache and `ContractService`, while sharing the CFAPI command bus, pending
registry, request ID sequence, and session. HTTP is served by Salvo 0.95.2 with lookup,
synchronization, health, OpenAPI 3.1, and Scalar routes.

## Configuration

Configuration comes from environment variables. A primary name takes precedence over its legacy
alias when both exist. Never put password values in command history, source control, logs, or this
document; inject them through the process supervisor or a secret manager.

### Required

| Primary variable | Legacy alias | Meaning |
| --- | --- | --- |
| `CFAPI_USERNAME` | `CFAPI_USER` | CFAPI username |
| `CFAPI_PASSWORD` | `CFAPI_PASS` | CFAPI password |
| `CFAPI_HOSTS` | `CFAPI_HOST` | Comma-separated `host:port` endpoints |

An explicitly present but empty primary variable is invalid and does not fall back to its alias.

### Optional

| Variable | Default | Meaning |
| --- | ---: | --- |
| `CONTRACT_HTTP_BIND` | `0.0.0.0:8080` | HTTP listen address |
| `CFAPI_SOURCES` | `533,534` | Source IDs; duplicates are removed |
| `CFAPI_MAX_USER_THREADS` | `0` | Maximum CFAPI user threads |
| `CFAPI_MAX_CSP_THREADS` | `32` | Maximum CFAPI CSP threads |
| `CFAPI_MAX_REQUEST_QUEUE_SIZE` | `100000` | Request queue size; minimum 100000 |
| `CFAPI_CONNECTION_COMPRESSION` | `true` | Connection compression |
| `CFAPI_COMMAND_CAPACITY` | `256` | Bounded command bus capacity |
| `CONTRACT_QUERY_TIMEOUT_MS` | `5000` | Query timeout |
| `CONTRACT_CACHE_FRESH_SECONDS` | `300` | Fresh cache lifetime |
| `CONTRACT_CACHE_MAX_STALE_SECONDS` | `86400` | Maximum stale lifetime; at least the fresh lifetime |
| `CONTRACT_MAX_EXACT_QUERIES` | `256` | Global concurrent exact-query limit |
| `CONTRACT_MAX_EXACT_PER_SOURCE` | `32` | Per-source exact-query limit; no greater than global limit |
| `CONTRACT_MAX_WHOLE_SOURCE_QUERIES` | `2` | Whole-source limit; at least the configured source count |
| `CONTRACT_ITEM_QUEUE_CAPACITY` | `4096` | Per-query item queue capacity |
| `CONTRACT_QUERY_MAX_RECORDS` | `100000` | Maximum records per query |
| `CONTRACT_QUERY_MAX_OWNED_BYTES` | `268435456` | Maximum owned callback bytes per query |
| `CONTRACT_TOMBSTONE_CAPACITY` | `65536` | Completed-request tombstone capacity |
| `CONTRACT_TOMBSTONE_TTL_SECONDS` | `360` | Tombstone lifetime |
| `CONTRACT_CACHE_MAX_RECORDS` | `100000` | Maximum records per source generation |
| `CONTRACT_CACHE_MAX_BYTES` | `268435456` | Maximum bytes per source generation |
| `CONTRACT_CACHE_MAX_OVERLAYS` | `10000` | Maximum exact-query overlays |
| `CONTRACT_NEGATIVE_TTL_SECONDS` | `30` | Negative cache lifetime |
| `CONTRACT_MAX_RETAINED_JOBS` | `1024` | Retained synchronization jobs |
| `CONTRACT_AUTO_SYNC` | `true` | Run startup whole-source syncs |
| `CONTRACT_SHUTDOWN_TIMEOUT_MS` | `10000` | Graceful HTTP shutdown timeout |

Values validated as nonzero must be greater than zero. Durations use seconds unless the variable
name ends in `_MS`.

## Run

After injecting the required environment variables, run from the repository root:

```bash
cargo run --offline -p contract-query-service --features runtime --bin contract-query-service
```

Examples below use the default bind port:

```bash
BASE=http://127.0.0.1:8080
```

## Health And Documentation

```bash
curl -i "$BASE/health/live"
curl -i "$BASE/health/ready"
curl -sS "$BASE/api-doc/openapi.json"
curl -i "$BASE/doc"
```

`/health/live` only confirms the HTTP process is responsive. `/health/ready` reports the CFAPI
session, cache, query coordinator, and sync coordinator independently.

## Lookups

```bash
curl -sS --get "$BASE/v1/contracts/by-source" \
  --data-urlencode 'source_id=533' \
  --data-urlencode 'symbol=AAPL' \
  --data-urlencode 'consistency=cache_preferred'

curl -sS --get "$BASE/v1/contracts/by-exchange" \
  --data-urlencode 'exchange=XNGS' \
  --data-urlencode 'code=AAPL' \
  --data-urlencode 'consistency=fresh_required'
```

`consistency` is optional and defaults to `cache_preferred`; the other accepted value is
`fresh_required`. Lookup responses include `ETag`, `Last-Modified`, `Cache-Control`, and
`x-request-id` headers.

## Synchronization And Recovery

With `CONTRACT_AUTO_SYNC=true`, each configured source runs one whole-source sync after the
transport session reports `SESSION_ESTABLISHED`. A failed startup sync leaves that cache
incomplete and is not retried by the startup task. Start a manual recovery job for each affected source:

```bash
curl -i -X POST "$BASE/v1/contract-sync-jobs" \
  -H 'Content-Type: application/json' \
  -H 'Idempotency-Key: recovery-533-001' \
  --data '{"source_id":533}'
```

The `202 Accepted` response includes `Location`. Poll its 26-character job ULID:

```bash
curl -sS "$BASE/v1/contract-sync-jobs/JOB_ULID"
```

Repeat for source 534, or every source in `CFAPI_SOURCES`. A successful sync atomically publishes a
complete generation. Cache readiness becomes ready only after every configured source cache is
complete; failed syncs do not mark it ready.

The CFAPI session becomes ready and opens the query gate when the transport reports
`SESSION_ESTABLISHED`; later `AVAILABLE` events keep it ready. `RECOVERY` or `UNAVAILABLE` closes
the gate, clears session readiness, and fails pending requests. Overall readiness
returns HTTP 503 until the session, every configured cache, query coordinator, and sync coordinator
are all ready. Liveness remains HTTP 200 while the HTTP process runs.

## Graceful Shutdown

Send `SIGINT` (normally Ctrl-C). The runtime marks the CFAPI session, query coordinator, and sync
coordinator not ready, fails pending work, aborts startup tasks, and asks Salvo to stop gracefully
within `CONTRACT_SHUTDOWN_TIMEOUT_MS`. It then shuts down the registry and waits for the HTTP server.
