# Domain Context

## Contract

Static and slowly changing metadata for one market instrument. A Contract does not contain tick or bid/ask events. A time-sensitive reference price may be attached as a separately observed value.

## Contract Key

The pair of Source ID and Symbol that uniquely identifies a Contract in CFAPI. A Symbol alone is not a Contract Key because the same text may exist on multiple sources.

## Source ID

The ICE `ENUM.SRC.ID` used to address CFAPI data. It is not a MIC and must not be presented as an exchange code.

## Symbol

The ICE `SYMBOL.TICKER` value. In the first contract schema, `code` and `symbol` are two public names for this same value.

## Exchange

The public contract field derived from `MIC.CODE_REF.MKT` (token 3241). It represents the listing/reference market used for the product contract. It must not be interpreted as the venue of an individual trade.

## Feed MIC

The separate `MIC.CODE` value from token 3240. It is retained independently because live data proves it can differ from Exchange and does not reliably identify the listing exchange.

## Reference Price

A market-data observation from token 7094. It is not QueryXref contract metadata and has its own observation time and freshness.

## Metadata Observation Time

The time at which a successful QueryXref result was accepted by the contract service. It is not an exchange-provided update timestamp.

## Refresh Generation

A complete candidate set of Contracts for one Source ID. It becomes visible only after the corresponding whole-source QueryXref completes successfully and all rows have been processed.

## Query Tag

The nonzero CFAPI-generated identifier that correlates one request with its callback events. It is generated before send and registered before callbacks can be delivered.

