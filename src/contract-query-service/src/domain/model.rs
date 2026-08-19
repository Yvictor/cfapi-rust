use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;
use time::OffsetDateTime;

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct ContractKey {
    pub source_id: u16,
    pub symbol: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Mic(String);

impl Mic {
    pub fn parse(value: &str) -> Option<Self> {
        (value.len() == 4
            && value
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit()))
        .then(|| Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Mic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CurrencyCode(String);

impl CurrencyCode {
    pub fn parse(value: &str) -> Option<Self> {
        (value.len() == 3 && value.bytes().all(|byte| byte.is_ascii_uppercase()))
            .then(|| Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContractSize {
    pub raw: String,
    pub value: Option<Decimal>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstrumentType {
    pub code: i32,
    pub label: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TickSizeBand {
    pub tick: Decimal,
    pub upper_bound: Option<Decimal>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TickSizeSchedule {
    pub raw: String,
    pub rules: Option<Vec<TickSizeBand>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Observed<T> {
    pub value: T,
    pub observed_at: OffsetDateTime,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GicsClassification {
    pub category: String,
    pub sector: String,
    pub industry: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContractMetadata {
    pub key: ContractKey,
    pub name: Option<String>,
    pub feed_mic: Option<Mic>,
    pub exchange: Option<Mic>,
    pub contract_size: Option<ContractSize>,
    pub currency: Option<CurrencyCode>,
    pub instrument_type: Option<InstrumentType>,
    pub tick_size: Option<TickSizeSchedule>,
    pub metadata_observed_at: OffsetDateTime,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContractView {
    pub metadata: ContractMetadata,
    pub reference: Option<Observed<Decimal>>,
    pub gics: Option<GicsClassification>,
}

impl ContractView {
    pub fn from_metadata(metadata: ContractMetadata) -> Self {
        Self {
            metadata,
            reference: None,
            gics: None,
        }
    }

    pub fn publication_exchange(&self) -> Result<&Mic, MissingPublicationExchange> {
        self.metadata
            .exchange
            .as_ref()
            .ok_or(MissingPublicationExchange)
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("contract has no token 3241 exchange and cannot use an exchange-addressed topic")]
pub struct MissingPublicationExchange;
