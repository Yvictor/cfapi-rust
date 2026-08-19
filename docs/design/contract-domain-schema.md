# Contract Domain Schema

Status: decision for issue #3

## Canonical Types

```rust
struct ContractKey {
    source_id: u16,
    symbol: String,
}

struct ContractMetadata {
    key: ContractKey,
    name: Option<String>,
    feed_mic: Option<Mic>,          // token 3240
    exchange: Option<Mic>,          // token 3241, listing/reference MIC
    contract_size: Option<ContractSize>,
    currency: Option<CurrencyCode>,
    instrument_type: Option<InstrumentType>,
    tick_size: Option<TickSizeSchedule>,
    metadata_observed_at: OffsetDateTime,
}

struct ContractView {
    metadata: ContractMetadata,
    reference: Option<Observed<Decimal>>,
    gics: Option<GicsClassification>,
}
```

The canonical key is `(source_id, symbol)`. `code` and `symbol` are equal in v1 and are expanded only in the wire DTO.

## Wire Fields

| Field | JSON / named MessagePack | Nullable | Source and rule |
|---|---|---:|---|
| `schema_version` | integer | no | `1` |
| `source_id` | integer | no | QueryXref source |
| `exchange` | string | yes | token 3241, listing/reference MIC |
| `feed_mic` | string | yes | token 3240, kept separate |
| `code` | string | no | event symbol/token 5 |
| `symbol` | string | no | equal to `code` in v1 |
| `name` | string | yes | token 3960 |
| `category` | string | yes | GICS Industry Group |
| `sector` | string | yes | GICS Sector |
| `industry` | string | yes | GICS Industry |
| `unit` | string | yes | raw token 3015 |
| `unit_value` | decimal string | yes | parsed token 3015 |
| `reference` | decimal string | yes | market-data state from token 7094 |
| `reference_observed_at` | RFC 3339 string | yes | observation time for `reference` |
| `currency` | string | yes | token 435, uppercase ISO 4217 when valid |
| `instrument_type_code` | integer | yes | raw token 3133 |
| `instrument_type` | string | yes | derived official label |
| `tick_size` | string | yes | raw token 3366 |
| `tick_size_rules` | array | yes | parsed tick bands |
| `update_date` | `YYYY-MM-DD` | no | UTC date of metadata observation |
| `metadata_observed_at` | RFC 3339 string | no | exact QueryXref observation time |

Decimal values are encoded as strings to avoid float loss and to match existing market payload precision. HTTP path version is `/v1`; MessagePack is a named map encoded with `rmp_serde::to_vec_named`. Adding nullable named fields is compatible within schema version 1; changing a field's name, type, or semantics requires a new version.

## MIC Decision

Public `exchange` remains token 3241, matching the supplied product field contract. It is named and documented as listing/reference MIC, not execution venue.

Token 3240 is not substituted for it. Live evidence shows source 534 symbol `A` returned `3240=XNAS` and `3241=XNYS`; using 3240 as exchange would classify that NYSE listing incorrectly. If 3241 is absent, the HTTP Contract remains valid with `exchange=null`, but it cannot be published to an exchange-addressed Solace topic.

## Parsing

### Contract Size

Preserve token 3015 as `unit`. Parse an optional derived Decimal from plain decimal values and uppercase `K`; for example `100K` becomes `100000`. Reject comma notation, scientific notation, unknown suffixes, non-positive values, and overflow. Parse failure leaves `unit_value=null`, retains the raw value, and increments a validation metric.

### Instrument Type

Keep token 3133 as an open `i32`, not a closed Rust enum. Resolve the official label through a versioned lookup table. Unknown future values retain `instrument_type_code` and produce `instrument_type=null`.

### Tick Size

```rust
struct TickSizeBand {
    tick: Decimal,
    upper_bound: Option<Decimal>,
}
```

The whitespace-delimited token list must contain an odd number of values. Tick values must be positive. Upper bounds must be positive and strictly increasing. The final tick has no upper bound. Invalid input retains raw `tick_size`, sets `tick_size_rules=null`, and does not reject the Contract.

## Independent Freshness

Token 7094 is not a QueryXref return field. `reference` and `reference_observed_at` come from a market-state adapter and may be null without making the Contract unavailable.

`update_date` is derived from `metadata_observed_at`; it is not presented as an exchange update date. GICS remains an optional enrichment adapter. It maps `category=Industry Group`, `sector=Sector`, and `industry=Industry`, and must use a stable identifier rather than ticker alone when one becomes available.

## Validation And Errors

- Missing optional QueryXref tokens return a valid Contract with null fields.
- Exact result source or symbol mismatch is a protocol violation and is not cached.
- Invalid MIC or currency retains the raw upstream evidence in diagnostics but does not enter the public Contract.
- QueryXref status 14 is not found. Permission, transport, timeout, and protocol failures remain distinct errors.
- The v1 invariant is `code == symbol`.

## Required Tests

- Same symbol on different sources and canonical-key isolation.
- MIC 3240/3241 disagreement and missing 3241 publication rejection.
- Contract sizes `100`, `1234.5`, `100K`, malformed and overflow values.
- Single and multi-band tick sizes, even token counts, zero ticks, and unordered bounds.
- Known and unknown 3133 values.
- Decimal JSON and named MessagePack golden round trips.
- Missing reference and GICS enrichment.
- UTC date-boundary behavior for observation timestamps.

