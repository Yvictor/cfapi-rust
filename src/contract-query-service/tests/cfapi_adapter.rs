#![cfg(feature = "cfapi")]

#[path = "../src/cfapi_adapter.rs"]
#[allow(dead_code)]
mod cfapi_adapter;

use cfapi::value::CFValue;
use cfapi_adapter::{
    build_owned_query_xref_row, owned_token_from_cf_value, CfapiAdapter, EventConversionError,
    QueryXrefEventBridge,
};
use contract_query_service::query::QueryError;
use contract_query_service::OwnedTokenValue;
use rust_decimal::Decimal;
use std::str::FromStr;
use time::macros::datetime;

fn assert_send<T: Send>() {}
fn assert_handler<T: cfapi::message_event::MessageEventHandlerExt>() {}
fn assert_owner<T: contract_query_service::service::CfapiCommandOwner>() {}

#[test]
fn adapter_and_bridge_satisfy_threading_contracts() {
    assert_send::<CfapiAdapter>();
    assert_owner::<CfapiAdapter>();
    assert_handler::<QueryXrefEventBridge>();
}

#[test]
fn converts_all_supported_cfapi_value_types() {
    let string = owned_token_from_cf_value(3960, CFValue::String("Apple".to_owned()))
        .unwrap()
        .unwrap();
    assert_eq!(string.value, OwnedTokenValue::String("Apple".to_owned()));

    let integer = owned_token_from_cf_value(3133, CFValue::Int(257))
        .unwrap()
        .unwrap();
    assert_eq!(integer.value, OwnedTokenValue::Integer(257));

    let decimal = owned_token_from_cf_value(901, CFValue::Double(12.25))
        .unwrap()
        .unwrap();
    assert_eq!(
        decimal.value,
        OwnedTokenValue::Decimal(Decimal::from_str("12.25").unwrap())
    );

    let datetime = owned_token_from_cf_value(902, CFValue::Datetime(1_700_000_000.5))
        .unwrap()
        .unwrap();
    assert_eq!(
        datetime.value,
        OwnedTokenValue::Decimal(Decimal::from_str("1700000000.5").unwrap())
    );
    assert!(owned_token_from_cf_value(903, CFValue::Unknown)
        .unwrap()
        .is_none());
}

#[test]
fn rejects_invalid_token_numbers_and_non_finite_numbers() {
    assert_eq!(
        owned_token_from_cf_value(-1, CFValue::Int(1)),
        Err(EventConversionError::InvalidTokenNumber(-1))
    );
    assert_eq!(
        owned_token_from_cf_value(8, CFValue::Double(f64::NAN)),
        Err(EventConversionError::NonFiniteDecimal { token: 8 })
    );
}

#[test]
fn builds_an_owned_row_without_borrowing_callback_memory() {
    let observed_at = datetime!(2026-08-19 10:00 UTC);
    let row = build_owned_query_xref_row(
        533,
        "AAPL".to_owned(),
        [
            (5, CFValue::String("AAPL".to_owned())),
            (3241, CFValue::String("XNAS".to_owned())),
        ],
        observed_at,
    )
    .unwrap()
    .unwrap();
    assert_eq!(row.source_id, 533);
    assert_eq!(row.symbol, "AAPL");
    assert_eq!(row.observed_at, observed_at);
    assert_eq!(row.tokens.len(), 2);
}

#[test]
fn zero_token_image_complete_with_symbol_has_no_final_row() {
    let row = build_owned_query_xref_row(
        533,
        "AAPL".to_owned(),
        Vec::<(i32, CFValue)>::new(),
        datetime!(2026-08-19 10:00 UTC),
    )
    .unwrap();
    assert!(row.is_none());
}

#[test]
fn rejects_out_of_range_sources() {
    assert_eq!(
        build_owned_query_xref_row(
            -1,
            "AAPL".to_owned(),
            Vec::<(i32, CFValue)>::new(),
            datetime!(2026-08-19 10:00 UTC),
        ),
        Err(EventConversionError::InvalidSource(-1))
    );
}

#[test]
fn public_error_type_remains_typed() {
    let error = QueryError::ProtocolViolation("bad QueryXref event".to_owned());
    assert!(matches!(error, QueryError::ProtocolViolation(_)));
}
