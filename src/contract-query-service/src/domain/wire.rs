use super::{ContractView, TickSizeBand};
use serde::{Deserialize, Serialize};
use time::{format_description::well_known::Rfc3339, Date, OffsetDateTime, UtcOffset};

pub const CONTRACT_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TickSizeBandDto {
    pub tick: String,
    pub upper_bound: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContractDto {
    pub schema_version: u16,
    pub source_id: u16,
    pub exchange: Option<String>,
    pub feed_mic: Option<String>,
    pub code: String,
    pub symbol: String,
    pub name: Option<String>,
    pub category: Option<String>,
    pub sector: Option<String>,
    pub industry: Option<String>,
    pub unit: Option<String>,
    pub unit_value: Option<String>,
    pub reference: Option<String>,
    pub reference_observed_at: Option<String>,
    pub currency: Option<String>,
    pub instrument_type_code: Option<i32>,
    pub instrument_type: Option<String>,
    pub tick_size: Option<String>,
    pub tick_size_rules: Option<Vec<TickSizeBandDto>>,
    pub update_date: String,
    pub metadata_observed_at: String,
}

impl ContractDto {
    pub fn from_view(view: &ContractView) -> Self {
        let metadata = &view.metadata;
        let contract_size = metadata.contract_size.as_ref();
        let instrument_type = metadata.instrument_type.as_ref();
        let tick_size = metadata.tick_size.as_ref();
        let gics = view.gics.as_ref();
        let reference = view.reference.as_ref();

        Self {
            schema_version: CONTRACT_SCHEMA_VERSION,
            source_id: metadata.key.source_id,
            exchange: metadata.exchange.as_ref().map(ToString::to_string),
            feed_mic: metadata.feed_mic.as_ref().map(ToString::to_string),
            code: metadata.key.symbol.clone(),
            symbol: metadata.key.symbol.clone(),
            name: metadata.name.clone(),
            category: gics.map(|value| value.category.clone()),
            sector: gics.map(|value| value.sector.clone()),
            industry: gics.map(|value| value.industry.clone()),
            unit: contract_size.map(|value| value.raw.clone()),
            unit_value: contract_size.and_then(|value| value.value.map(|value| value.to_string())),
            reference: reference.map(|value| value.value.to_string()),
            reference_observed_at: reference.map(|value| format_timestamp(value.observed_at)),
            currency: metadata
                .currency
                .as_ref()
                .map(|value| value.as_str().to_owned()),
            instrument_type_code: instrument_type.map(|value| value.code),
            instrument_type: instrument_type.and_then(|value| value.label.map(str::to_owned)),
            tick_size: tick_size.map(|value| value.raw.clone()),
            tick_size_rules: tick_size
                .and_then(|value| value.rules.as_ref())
                .map(|rules| rules.iter().map(TickSizeBandDto::from).collect()),
            update_date: format_date(
                metadata
                    .metadata_observed_at
                    .to_offset(UtcOffset::UTC)
                    .date(),
            ),
            metadata_observed_at: format_timestamp(metadata.metadata_observed_at),
        }
    }

    pub fn to_named_msgpack(&self) -> Result<Vec<u8>, rmp_serde::encode::Error> {
        rmp_serde::to_vec_named(self)
    }

    pub fn from_msgpack(bytes: &[u8]) -> Result<Self, rmp_serde::decode::Error> {
        rmp_serde::from_slice(bytes)
    }
}

impl From<&TickSizeBand> for TickSizeBandDto {
    fn from(value: &TickSizeBand) -> Self {
        Self {
            tick: value.tick.to_string(),
            upper_bound: value.upper_bound.map(|bound| bound.to_string()),
        }
    }
}

fn format_timestamp(value: OffsetDateTime) -> String {
    value
        .to_offset(UtcOffset::UTC)
        .format(&Rfc3339)
        .expect("OffsetDateTime always supports RFC 3339 formatting")
}

fn format_date(value: Date) -> String {
    let (year, month, day) = value.to_calendar_date();
    format!("{year:04}-{:02}-{day:02}", month as u8)
}
