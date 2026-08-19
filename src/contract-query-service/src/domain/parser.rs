use super::{
    ContractKey, ContractMetadata, ContractSize, ContractView, CurrencyCode, InstrumentType, Mic,
    TickSizeBand, TickSizeSchedule,
};
use rust_decimal::Decimal;
use std::{collections::HashMap, str::FromStr};
use thiserror::Error;
use time::OffsetDateTime;

pub const TOKEN_SYMBOL: u16 = 5;
pub const TOKEN_CURRENCY: u16 = 435;
pub const TOKEN_CONTRACT_SIZE: u16 = 3015;
pub const TOKEN_INSTRUMENT_TYPE: u16 = 3133;
pub const TOKEN_FEED_MIC: u16 = 3240;
pub const TOKEN_EXCHANGE: u16 = 3241;
pub const TOKEN_TICK_SIZE: u16 = 3366;
pub const TOKEN_NAME: u16 = 3960;

pub const INSTRUMENT_TYPE_ENUM_VERSION: &str = "ICE-CF-2024-05-03";

#[derive(Clone, Debug, PartialEq)]
pub enum OwnedTokenValue {
    Integer(i64),
    Decimal(Decimal),
    String(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct OwnedToken {
    pub number: u16,
    pub value: OwnedTokenValue,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OwnedQueryXrefRow {
    pub source_id: u16,
    pub symbol: String,
    pub tokens: Vec<OwnedToken>,
    pub observed_at: OffsetDateTime,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValidationIssue {
    DuplicateToken(u16),
    InvalidMic { token: u16, raw: String },
    InvalidCurrency { raw: String },
    InvalidContractSize { raw: String },
    InvalidInstrumentType { raw: String },
    InvalidTickSize { raw: String },
    UnexpectedTokenType { token: u16 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContractParseResult {
    pub contract: ContractView,
    pub issues: Vec<ValidationIssue>,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ContractRowError {
    #[error("QueryXref source mismatch: expected {expected}, received {actual}")]
    SourceMismatch { expected: u16, actual: u16 },
    #[error("QueryXref symbol mismatch: expected {expected:?}, received {actual:?}")]
    SymbolMismatch { expected: String, actual: String },
    #[error("QueryXref row has an empty symbol")]
    EmptySymbol,
}

pub fn parse_contract_size(raw: &str) -> Option<Decimal> {
    if raw.is_empty() || raw.contains([',', 'e', 'E']) {
        return None;
    }

    let (number, multiplier) = match raw.strip_suffix('K') {
        Some(number) if !number.is_empty() => (number, Decimal::from(1_000)),
        Some(_) => return None,
        None => (raw, Decimal::ONE),
    };
    let value = Decimal::from_str(number).ok()?.checked_mul(multiplier)?;
    (value > Decimal::ZERO).then_some(value)
}

pub fn instrument_type_label(code: i32) -> Option<&'static str> {
    Some(match code {
        0 => "UNDEFINED",
        256 => "EQUITY",
        257 => "COMMON_STOCK",
        258 => "PREFERRED_STOCK",
        260 => "WARRANT",
        264 => "PREMIUM",
        268 => "TRUST",
        270 => "RIGHT",
        271 => "WARRANT_RIGHT",
        512 => "INDEX",
        768 => "UNIT",
        860 => "UNITS_OF_BENEFICIAL_INTEREST",
        1024 => "COMMODITY",
        1025 => "FUTURE_SPREAD",
        1280 => "FUTURE",
        1290 => "FORWARD",
        1306 => "SPOT",
        1536 => "DEPOSITORY_RECEIPT",
        2048 => "OPTION",
        2049 => "OPTION SPREAD",
        2304 => "EQUITY_OPTION",
        2560 => "INDEX_OPTION",
        2816 => "BINARY_OPTION",
        3072 => "FUTURE_OPTION",
        3328 => "COMMERCIAL_PAPER",
        3584 => "LIMITED_PARTNERSHIP",
        3840 => "NOTE",
        4096 => "FIXED_INCOME",
        4097 => "BOND",
        4098 => "CONVERTIBLE_BOND",
        4099 => "MORTGAGE_BACKED_SECURITY",
        4100 => "GOV_BOND",
        4104 => "CORP_BOND",
        4112 => "US_AGENCY_BOND",
        4128 => "TREASURY_BILL",
        4160 => "US_TREASURY_COUPON",
        4224 => "MONEY_MARKET",
        4352 => "CD",
        5000 => "STRUCTURED_PRODUCT",
        5100 => "SWAP",
        5102 => "INTEREST_RATE_SWAP",
        5104 => "FX_SWAP",
        5106 => "CREDIT_DEFAULT_SWAP",
        5108 => "ASSET_SWAP",
        5110 => "FUTURES_SWAP",
        5112 => "TOTAL_RETURN_SWAP",
        5194 => "SEC_144A",
        6100 => "STRATEGY",
        8192 => "MUTUAL_FUND",
        8193 => "EXCHANGE_TRADED_FUND",
        8194 => "EXCHANGE_TRADED_NOTE",
        8195 => "EXCHANGE_TRADED_COMMODITY",
        8196 => "CLOSED_END_FUND",
        8197 => "EXCHANGE_TRADED_PRODUCT",
        8200 => "REIT",
        _ => return None,
    })
}

pub fn parse_tick_size_schedule(raw: &str) -> Option<Vec<TickSizeBand>> {
    let values = raw
        .split_whitespace()
        .map(|value| Decimal::from_str(value).ok())
        .collect::<Option<Vec<_>>>()?;
    if values.is_empty() || values.len() % 2 == 0 {
        return None;
    }

    let mut rules = Vec::with_capacity(values.len().div_ceil(2));
    let mut previous_upper = None;
    for pair in values.chunks(2) {
        let tick = pair[0];
        if tick <= Decimal::ZERO {
            return None;
        }
        let upper_bound = pair.get(1).copied();
        if let Some(upper) = upper_bound {
            if upper <= Decimal::ZERO || previous_upper.is_some_and(|previous| upper <= previous) {
                return None;
            }
            previous_upper = Some(upper);
        }
        rules.push(TickSizeBand { tick, upper_bound });
    }
    Some(rules)
}

pub fn convert_query_xref_row(
    row: OwnedQueryXrefRow,
    expected_key: Option<&ContractKey>,
) -> Result<ContractParseResult, ContractRowError> {
    if row.symbol.is_empty() {
        return Err(ContractRowError::EmptySymbol);
    }
    if let Some(expected) = expected_key {
        if row.source_id != expected.source_id {
            return Err(ContractRowError::SourceMismatch {
                expected: expected.source_id,
                actual: row.source_id,
            });
        }
        if row.symbol != expected.symbol {
            return Err(ContractRowError::SymbolMismatch {
                expected: expected.symbol.clone(),
                actual: row.symbol,
            });
        }
    }

    let key = ContractKey {
        source_id: row.source_id,
        symbol: row.symbol,
    };
    let mut issues = Vec::new();
    let mut tokens = HashMap::new();
    for token in row.tokens {
        if tokens.insert(token.number, token.value).is_some() {
            issues.push(ValidationIssue::DuplicateToken(token.number));
        }
    }

    if let Some(token_symbol) = tokens.get(&TOKEN_SYMBOL) {
        match token_symbol {
            OwnedTokenValue::String(token_symbol) if token_symbol != &key.symbol => {
                return Err(ContractRowError::SymbolMismatch {
                    expected: key.symbol,
                    actual: token_symbol.clone(),
                });
            }
            OwnedTokenValue::String(_) => {}
            _ => issues.push(ValidationIssue::UnexpectedTokenType {
                token: TOKEN_SYMBOL,
            }),
        }
    }

    let name = take_string(&mut tokens, TOKEN_NAME, &mut issues);
    let feed_mic = take_validated_string(&mut tokens, TOKEN_FEED_MIC, &mut issues, |raw| {
        Mic::parse(raw).ok_or_else(|| ValidationIssue::InvalidMic {
            token: TOKEN_FEED_MIC,
            raw: raw.to_owned(),
        })
    });
    let exchange = take_validated_string(&mut tokens, TOKEN_EXCHANGE, &mut issues, |raw| {
        Mic::parse(raw).ok_or_else(|| ValidationIssue::InvalidMic {
            token: TOKEN_EXCHANGE,
            raw: raw.to_owned(),
        })
    });
    let currency = take_validated_string(&mut tokens, TOKEN_CURRENCY, &mut issues, |raw| {
        CurrencyCode::parse(raw).ok_or_else(|| ValidationIssue::InvalidCurrency {
            raw: raw.to_owned(),
        })
    });
    let contract_size = take_string(&mut tokens, TOKEN_CONTRACT_SIZE, &mut issues).map(|raw| {
        let value = parse_contract_size(&raw);
        if value.is_none() {
            issues.push(ValidationIssue::InvalidContractSize { raw: raw.clone() });
        }
        ContractSize { raw, value }
    });
    let instrument_type = take_instrument_type(&mut tokens, &mut issues);
    let tick_size = take_string(&mut tokens, TOKEN_TICK_SIZE, &mut issues).map(|raw| {
        let rules = parse_tick_size_schedule(&raw);
        if rules.is_none() {
            issues.push(ValidationIssue::InvalidTickSize { raw: raw.clone() });
        }
        TickSizeSchedule { raw, rules }
    });

    Ok(ContractParseResult {
        contract: ContractView::from_metadata(ContractMetadata {
            key,
            name,
            feed_mic,
            exchange,
            contract_size,
            currency,
            instrument_type,
            tick_size,
            metadata_observed_at: row.observed_at,
        }),
        issues,
    })
}

fn take_string(
    tokens: &mut HashMap<u16, OwnedTokenValue>,
    number: u16,
    issues: &mut Vec<ValidationIssue>,
) -> Option<String> {
    match tokens.remove(&number) {
        Some(OwnedTokenValue::String(value)) => Some(value),
        Some(_) => {
            issues.push(ValidationIssue::UnexpectedTokenType { token: number });
            None
        }
        None => None,
    }
}

fn take_validated_string<T>(
    tokens: &mut HashMap<u16, OwnedTokenValue>,
    number: u16,
    issues: &mut Vec<ValidationIssue>,
    parse: impl FnOnce(&str) -> Result<T, ValidationIssue>,
) -> Option<T> {
    let raw = take_string(tokens, number, issues)?;
    match parse(&raw) {
        Ok(value) => Some(value),
        Err(issue) => {
            issues.push(issue);
            None
        }
    }
}

fn take_instrument_type(
    tokens: &mut HashMap<u16, OwnedTokenValue>,
    issues: &mut Vec<ValidationIssue>,
) -> Option<InstrumentType> {
    let code = match tokens.remove(&TOKEN_INSTRUMENT_TYPE) {
        Some(OwnedTokenValue::Integer(value)) => i32::try_from(value).ok().or_else(|| {
            issues.push(ValidationIssue::InvalidInstrumentType {
                raw: value.to_string(),
            });
            None
        }),
        Some(OwnedTokenValue::String(raw)) => raw.parse::<i32>().ok().or_else(|| {
            issues.push(ValidationIssue::InvalidInstrumentType { raw });
            None
        }),
        Some(_) => {
            issues.push(ValidationIssue::UnexpectedTokenType {
                token: TOKEN_INSTRUMENT_TYPE,
            });
            None
        }
        None => None,
    }?;
    Some(InstrumentType {
        code,
        label: instrument_type_label(code),
    })
}
