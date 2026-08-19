# QueryXref Contract Data Research

Date: 2026-08-19

## Decision

Use CFAPI `QUERYXREF` as the source for static contract metadata, with exact-symbol queries for cache misses and source-wide queries for refresh jobs. Correlate every request by the tag returned from `Session::send` and complete it only on the matching `IMAGE_COMPLETE` or terminal status.

Do not model every requested product field directly from the supplied field sheet. The live value types and the official QueryXref data model require the schema adjustments below.

## Official Contract

The ICE API Developer Guide, section 7.5.4, defines `QUERYXREF` as the reference-data query. `ENUM.SRC.ID` and `SYMBOL.TICKER` identify an exact instrument. The Consolidated Feed Wire Protocol Reference Data Services guide, "Querying the CSP for Different Datasets", states that omitting `SYMBOL.TICKER` returns all tickers on the source.

`SELECTUSERFILTERTOKENS` applies per source to future responses. Source and symbol remain present even when they are not included in the selected token list.

The response contract always includes query status and tag. The API event model defines `IMAGE_PART` as an intermediate result and `IMAGE_COMPLETE` as the final or only result. The developer guide's handler example uses `MessageEvent::getTag()` to correlate terminal events with requests.

Typical protocol errors documented for QueryXref are:

| Status | Meaning |
|---|---|
| `-46` | Invalid or missing command |
| `-36` | Missing `ENUM.SRC.ID` |
| `-12` | CSP not entitled for the requested permission |

## Live Validation

Environment: canned-data CSP account, CFAPI 1.7 binding, sources 533 and 534. No credentials are recorded in this document.

### Exact Queries

| Query | Event | Status | Result |
|---|---|---:|---|
| 533 / AAPL | `IMAGE_COMPLETE` | 0 | Product returned in the terminal event |
| 533 / A | `IMAGE_COMPLETE` | 14 | `WARN_NO_DATA_FOUND` |
| 534 / A | `IMAGE_COMPLETE` | 0 | Product returned in the terminal event |
| 534 / AAPL | `IMAGE_COMPLETE` | 14 | `WARN_NO_DATA_FOUND` |
| 533 / nonexistent symbol | `IMAGE_COMPLETE` | 14 | `WARN_NO_DATA_FOUND` |
| 558 / AAPL | `IMAGE_COMPLETE` | 14 | `WARN_NO_DATA_FOUND`; this does not prove whether the cause is entitlement or absent data |

Callbacks can arrive out of send order, but every callback carried the exact tag returned by the corresponding send call. A successful exact query places the product data on `IMAGE_COMPLETE`; therefore terminal events cannot be discarded before inspecting their symbol and tokens.

### Whole-Source Queries

Counts are observations from the canned-data environment and can change as its universe changes.

| Source | Product events | Terminal event | Shape |
|---|---:|---|---|
| 533 | 6,234 `IMAGE_PART` | one empty-symbol `IMAGE_COMPLETE` | all events used one query tag |
| 534 | 8,053 `IMAGE_PART` | one empty-symbol `IMAGE_COMPLETE` | all events used one query tag |

This confirms that a refresh job can stream products directly into a staging cache and atomically publish the new source snapshot after the matching complete event. It should not retain the entire CFAPI response in memory.

### Requested Fields

| API field | Token/source | Official type | Live examples | Required handling |
|---|---|---|---|---|
| `code`, `symbol` | event symbol / 5 | String | `AAPL`, `A` | Use the event symbol; token 5 is implicit key data |
| `name` | 3960 | String | `Apple Inc.` | Nullable String |
| `currency` | 435 | String | `USD` | Nullable String |
| `unit` | 3015 | **String** | `40`, `100` | Preserve String initially; valid protocol values include decimals and suffixes such as `100K`, so `int` is unsafe |
| `instrument_type` | 3133 | **Integer enum** | 257, 8193 | Preserve integer code; add a separately derived label only after loading the official enumeration |
| `exchange` | 3241 | String | `XNGS`, `XNYS` | This is `MIC.CODE_REF.MKT`, not necessarily the trading venue; expose semantics explicitly |
| operating MIC | 3240 | String | `XNAS` | Keep separately from 3241; do not silently substitute one for the other |
| `tick_size` | 3366 | String | `0.0001 1 0.01` | Preserve raw regime string and parse into bands in a derived representation |
| `reference` | 7094 | not listed as a QueryXref return field | absent from successful AAPL and A queries | Populate from market-data snapshot/update state or leave null; do not make QueryXref refresh depend on it |
| GICS fields | external dataset | n/a | not requested from these sources | Nullable until the licensed enrichment source is defined |
| `update_date` | service metadata | n/a | n/a | Set from cache refresh time, not a fabricated exchange date |

The filter command accepted tokens 435, 3015, 3133, 3240, 3241, 3366, 3960, and 7094. Acceptance only means the token is recognized; it does not guarantee a value for a product.

## Implementation Consequences

1. Extend the Rust binding with a dedicated QueryXref method that returns `i64` query tags and accepts an optional symbol. The current generic `request` method discards the tag and always adds a symbol, so it cannot support safe correlation or whole-source refresh.
2. Use a concurrent pending-query registry keyed by tag. Register completion state before responses can be consumed, and tolerate callbacks arriving in a different order from sends.
3. For exact queries, parse a non-empty-symbol `IMAGE_COMPLETE` as both the product and completion. Map status 14 to not found. Keep entitlement and transport failures distinct when the CSP supplies distinct codes.
4. For whole-source refresh, stream each `IMAGE_PART` into a generation-specific staging cache. Commit only after the empty-symbol `IMAGE_COMPLETE` with status 0. On timeout, disconnect, or error, discard the staging generation and retain the prior cache.
5. Keep the HTTP domain type honest: `unit: Option<String>`, `instrument_type: Option<i64>`, separate `mic` and `reference_mic`, and `reference: Option<f64>` supplied by a different market-data state source.
6. Apply `SELECTUSERFILTERTOKENS` once per source after session establishment, before QueryXref traffic. Include only the fields required by the contract service.

## Open Decisions For Follow-Up Tickets

- Decide whether the public field currently named `exchange` means operating MIC (3240) or reference/listing MIC (3241). The supplied field sheet selects 3241, but that is not a general trading-venue field.
- Decide whether `unit` remains a raw string in the public HTTP schema or is exposed as a parsed decimal plus original value.
- Load and version the 3133 enumeration before promising a string `instrument_type` label.
- Define the market-data state interface that supplies token 7094 independently of QueryXref.
- Validate a real `ERR_AUTH_REQ (-12)` response against a known non-entitled permission; the source 558 probe returned only no-data status 14.

## Sources

- *ICE Data Services API Developer's Guide v1.21.2*, sections 5.3 (`MessageEvent`), 7.4.3 (`SELECTUSERFILTERTOKENS`), and 7.5.4 (`QUERYXREF`). Local project file: `docs/ICE-Data-Services- API -Developer-Guide_v.1.21.2_20260108.pdf`.
- *Consolidated Feed Wire Protocol Data Model Reference Data Services User Guide*, sections "Querying the CSP for Different Datasets (using QueryXref)", "Typical Errors and Response Codes", and "Tokens that can be returned by QueryXRef". Official document dated 2024-04-03.
- User-supplied field contract, `product_fields.md`, received 2026-08-19.
