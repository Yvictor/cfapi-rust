# Salvo And OpenAPI Pattern

Status: decision for issue #8

## Decision

Create an independent workspace binary crate at `src/contract-query-service`. Reuse `cfapi` through an application/service seam; do not add HTTP behavior to cfvhub.

Use Salvo 0.89 with `oapi`, `affix-state`, `logging`, `request-id`, `catch-panic`, and `size-limiter`. Salvo 0.89 requires Rust 1.89; verify the deployment toolchain before implementation. The crate may remain edition 2021.

## Layout

```text
src/contract-query-service/
  Cargo.toml
  src/
    main.rs
    app.rs
    config.rs
    error.rs
    state.rs
    health.rs
    http/{mod,routes,handlers,models}.rs
    service/{contract_query,contract_sync}.rs
```

HTTP handlers only extract and validate input, call the application module, and render typed responses. CFAPI correlation, caching, and synchronization stay below this interface.

## Salvo Composition

1. Build the versioned API router.
2. Build `OpenApi` and merge the API router before adding documentation routes.
3. Serve JSON at `/api-doc/openapi.json` and Scalar at `/doc`.
4. Install outer hoops in the order CatchPanic, RequestId, tracing Logger, and typed state injection.
5. Install a custom Catcher for JSON 404, 405, and 500 responses.
6. Build `Service` with that router and catcher.

Use `affix_state::inject(AppState)` and `depot.obtain::<AppState>()`; do not use string Depot keys. Missing state is a typed internal-configuration error, never an unwrap panic.

DTOs derive `Serialize`, `Deserialize`, and `ToSchema`. Endpoints use `#[endpoint]`, typed path parameters, and typed JSON bodies. The final routes are owned by issue #4; the required surfaces are contract read, source sync creation/status, liveness, readiness, OpenAPI JSON, and Scalar.

## Typed Errors

`ApiErrorResponse` implements OpenAPI responses and Salvo response writing. Every failure sets the real HTTP status and returns:

```json
{
  "code": "contract_not_found",
  "message": "contract was not found",
  "request_id": "01..."
}
```

| Status | Use |
|---:|---|
| 400 | malformed source, symbol, MIC, or body |
| 404 | contract not found |
| 409 | source sync already running |
| 422 | unsupported source or permission denied |
| 502 | upstream CFAPI status or protocol violation |
| 503 | not ready, disconnected, shutting down, or local backpressure |
| 504 | upstream query timeout |
| 500 | internal invariant or configuration error |

Application failures must never be encoded in a successful HTTP 200 response.

## Runtime Policy

- Preserve an inbound `x-request-id` or generate one, and return it in the response.
- Log method, normalized route, status, latency, and request ID. Do not log credentials, full Contract responses, or raw CFAPI payloads.
- Limit sync mutation bodies to 8 KiB initially.
- Apply exact-query timeout at the application module so pending cleanup runs. Do not use a global Salvo timeout that merely drops the handler future.
- Parse configuration strictly at startup. Secrets have no defaults, are wrapped in secrecy types, and are not loaded from `.env` in production.
- Use only required Tokio features rather than `full`.

## Health And Shutdown

`/health/live` verifies only the process/event loop and responds quickly. `/health/ready` reads local state: CFAPI session established, initial cache usable, coordinators open, and queues below refusal thresholds. It never performs an upstream query. Not-ready is HTTP 503.

On SIGTERM or Ctrl-C: mark readiness false, reject new query/sync work, call graceful HTTP stop with a bounded timeout, wait for accepted requests, then close CFAPI and background workers.

## Reference Project Evaluation

Adopt from `api_idc_out:/ap/repo/ddb-dwt`: `#[endpoint]`, `ToSchema`, OpenAPI/Scalar routes, state injection, structured tracing, and dependency-aware health checks.

Reject or replace: HTTP 200 application errors, HTTP 200 unhealthy responses, string Depot keys and unwrap, default credentials, lenient config parsing, payload logging, Tokio `full`, String-only errors, missing request IDs/body limits, production `.env`, and absent graceful shutdown.

## Required Tests

Use `salvo::test::TestClient` with fake query and sync modules. Cover every error status/code/body, success/not-found/timeout/disconnect/backpressure, readiness transitions, malformed and oversized input, request-ID preservation and generation, OpenAPI snapshots including error responses, cancellation cleanup after timeout, and graceful shutdown behavior.

