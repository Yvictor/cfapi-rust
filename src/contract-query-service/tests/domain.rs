use contract_query_service::*;
use rust_decimal::Decimal;
use std::str::FromStr;
use time::macros::datetime;

fn token(number: u16, value: impl Into<String>) -> OwnedToken {
    OwnedToken {
        number,
        value: OwnedTokenValue::String(value.into()),
    }
}

fn sample_row(source_id: u16, symbol: &str) -> OwnedQueryXrefRow {
    OwnedQueryXrefRow {
        source_id,
        symbol: symbol.to_owned(),
        observed_at: datetime!(2026-08-19 23:59:59.123456789 UTC),
        tokens: vec![
            token(TOKEN_SYMBOL, symbol),
            token(TOKEN_NAME, "Apple Inc."),
            token(TOKEN_FEED_MIC, "XNAS"),
            token(TOKEN_EXCHANGE, "XNYS"),
            token(TOKEN_CONTRACT_SIZE, "100K"),
            token(TOKEN_CURRENCY, "USD"),
            OwnedToken {
                number: TOKEN_INSTRUMENT_TYPE,
                value: OwnedTokenValue::Integer(257),
            },
            token(TOKEN_TICK_SIZE, "0.001 10 0.005 50 0.01"),
        ],
    }
}

#[test]
fn contract_key_isolated_by_source() {
    let first = ContractKey {
        source_id: 533,
        symbol: "AAPL".to_owned(),
    };
    let second = ContractKey {
        source_id: 534,
        symbol: "AAPL".to_owned(),
    };
    assert_ne!(first, second);
}

#[test]
fn converts_owned_query_xref_row_and_separates_mics() {
    let result = convert_query_xref_row(sample_row(534, "A"), None).unwrap();
    let metadata = result.contract.metadata;

    assert_eq!(metadata.feed_mic.unwrap().as_str(), "XNAS");
    assert_eq!(metadata.exchange.unwrap().as_str(), "XNYS");
    assert_eq!(
        metadata.contract_size.unwrap().value,
        Some(Decimal::from(100_000))
    );
    assert!(result.issues.is_empty());
}

#[test]
fn exact_conversion_rejects_source_and_symbol_mismatch() {
    let expected = ContractKey {
        source_id: 533,
        symbol: "AAPL".to_owned(),
    };
    assert!(matches!(
        convert_query_xref_row(sample_row(534, "AAPL"), Some(&expected)),
        Err(ContractRowError::SourceMismatch { .. })
    ));
    assert!(matches!(
        convert_query_xref_row(sample_row(533, "MSFT"), Some(&expected)),
        Err(ContractRowError::SymbolMismatch { .. })
    ));
}

#[test]
fn invalid_optional_values_are_retained_as_issues_not_fatal_errors() {
    let mut row = sample_row(533, "AAPL");
    row.tokens = vec![
        token(TOKEN_EXCHANGE, "nyse"),
        token(TOKEN_CURRENCY, "US"),
        token(TOKEN_CONTRACT_SIZE, "1M"),
        token(TOKEN_TICK_SIZE, "0 10 0.1"),
    ];
    let result = convert_query_xref_row(row, None).unwrap();

    assert!(result.contract.metadata.exchange.is_none());
    assert!(result.contract.metadata.currency.is_none());
    assert_eq!(result.contract.metadata.contract_size.unwrap().raw, "1M");
    assert_eq!(result.contract.metadata.tick_size.unwrap().raw, "0 10 0.1");
    assert_eq!(result.issues.len(), 4);
}

#[test]
fn duplicate_tokens_are_reported_and_last_value_wins() {
    let mut row = sample_row(533, "AAPL");
    row.tokens.push(token(TOKEN_NAME, "Apple Incorporated"));
    let result = convert_query_xref_row(row, None).unwrap();

    assert_eq!(
        result.contract.metadata.name.as_deref(),
        Some("Apple Incorporated")
    );
    assert!(result
        .issues
        .contains(&ValidationIssue::DuplicateToken(TOKEN_NAME)));
}

#[test]
fn parses_contract_size_decimal_and_uppercase_k_only() {
    assert_eq!(parse_contract_size("100"), Some(Decimal::from(100)));
    assert_eq!(
        parse_contract_size("1234.5"),
        Decimal::from_str("1234.5").ok()
    );
    assert_eq!(parse_contract_size("100K"), Some(Decimal::from(100_000)));
    for invalid in ["", "0", "-1", "1,000", "1e3", "1k", "1M", "K"] {
        assert_eq!(parse_contract_size(invalid), None, "accepted {invalid}");
    }
    assert_eq!(parse_contract_size("9999999999999999999999999999K"), None);
}

#[test]
fn instrument_type_lookup_is_open_and_versioned() {
    assert_eq!(instrument_type_label(257), Some("COMMON_STOCK"));
    assert_eq!(instrument_type_label(8193), Some("EXCHANGE_TRADED_FUND"));
    assert_eq!(instrument_type_label(8200), Some("REIT"));
    assert_eq!(instrument_type_label(99_999), None);
    assert_eq!(INSTRUMENT_TYPE_ENUM_VERSION, "ICE-CF-2024-05-03");
}

#[test]
fn parses_single_and_multiple_tick_bands() {
    assert_eq!(
        parse_tick_size_schedule("0.01"),
        Some(vec![TickSizeBand {
            tick: Decimal::from_str("0.01").unwrap(),
            upper_bound: None,
        }])
    );
    let rules = parse_tick_size_schedule("0.001 10 0.005 50 0.01 100 0.05").unwrap();
    assert_eq!(rules.len(), 4);
    assert_eq!(rules[0].upper_bound, Some(Decimal::from(10)));
    assert_eq!(rules[3].upper_bound, None);
}

#[test]
fn rejects_invalid_tick_schedules() {
    for invalid in [
        "",
        "0.01 10",
        "0 10 0.01",
        "0.01 0 0.02",
        "0.01 50 0.02 10 0.03",
        "0.01 x 0.02",
    ] {
        assert_eq!(
            parse_tick_size_schedule(invalid),
            None,
            "accepted {invalid}"
        );
    }
}

#[test]
fn missing_reference_and_gics_are_valid_and_utc_date_is_derived() {
    let view = convert_query_xref_row(sample_row(533, "AAPL"), None)
        .unwrap()
        .contract;
    let dto = ContractDto::from_view(&view);

    assert!(dto.reference.is_none());
    assert!(dto.reference_observed_at.is_none());
    assert!(dto.category.is_none());
    assert_eq!(dto.update_date, "2026-08-19");
    assert_eq!(dto.metadata_observed_at, "2026-08-19T23:59:59.123456789Z");
}

#[test]
fn missing_3241_is_valid_for_http_but_rejected_for_exchange_publication() {
    let mut row = sample_row(533, "AAPL");
    row.tokens.retain(|token| token.number != TOKEN_EXCHANGE);
    let view = convert_query_xref_row(row, None).unwrap().contract;

    assert!(view.metadata.exchange.is_none());
    assert_eq!(view.publication_exchange(), Err(MissingPublicationExchange));
}

#[test]
fn wire_dto_includes_independently_observed_reference_and_gics() {
    let mut view = convert_query_xref_row(sample_row(533, "AAPL"), None)
        .unwrap()
        .contract;
    view.reference = Some(Observed {
        value: Decimal::from_str("225.125").unwrap(),
        observed_at: datetime!(2026-08-20 00:00:01 UTC),
    });
    view.gics = Some(GicsClassification {
        category: "Technology Hardware & Equipment".to_owned(),
        sector: "Information Technology".to_owned(),
        industry: "Technology Hardware, Storage & Peripherals".to_owned(),
    });
    let dto = ContractDto::from_view(&view);

    assert_eq!(dto.reference.as_deref(), Some("225.125"));
    assert_eq!(
        dto.reference_observed_at.as_deref(),
        Some("2026-08-20T00:00:01Z")
    );
    assert_eq!(dto.sector.as_deref(), Some("Information Technology"));
}

#[test]
fn update_date_uses_utc_boundary_not_input_offset() {
    let mut row = sample_row(533, "AAPL");
    row.observed_at = datetime!(2026-08-20 00:30:00 +02:00);
    let dto = ContractDto::from_view(&convert_query_xref_row(row, None).unwrap().contract);

    assert_eq!(dto.update_date, "2026-08-19");
    assert_eq!(dto.metadata_observed_at, "2026-08-19T22:30:00Z");
}

#[test]
fn json_decimal_fields_are_strings_and_code_equals_symbol() {
    let view = convert_query_xref_row(sample_row(533, "AAPL"), None)
        .unwrap()
        .contract;
    let dto = ContractDto::from_view(&view);
    let json = serde_json::to_value(&dto).unwrap();

    assert_eq!(json["code"], "AAPL");
    assert_eq!(json["symbol"], "AAPL");
    assert_eq!(json["unit_value"], "100000");
    assert_eq!(json["tick_size_rules"][0]["tick"], "0.001");
}

#[test]
fn named_messagepack_round_trip_and_golden_payload() {
    let view = convert_query_xref_row(sample_row(533, "AAPL"), None)
        .unwrap()
        .contract;
    let dto = ContractDto::from_view(&view);
    let bytes = dto.to_named_msgpack().unwrap();

    assert_eq!(ContractDto::from_msgpack(&bytes).unwrap(), dto);
    assert_eq!(
        hex(&bytes),
        concat!(
            "de0015ae736368656d615f76657273696f6e01a9736f757263655f6964cd0215",
            "a865786368616e6765a4584e5953a8666565645f6d6963a4584e4153a4636f64",
            "65a44141504ca673796d626f6ca44141504ca46e616d65aa4170706c6520496e",
            "632ea863617465676f7279c0a6736563746f72c0a8696e647573747279c0a475",
            "6e6974a43130304baa756e69745f76616c7565a6313030303030a97265666572",
            "656e6365c0b57265666572656e63655f6f627365727665645f6174c0a8637572",
            "72656e6379a3555344b4696e737472756d656e745f747970655f636f6465cd01",
            "01af696e737472756d656e745f74797065ac434f4d4d4f4e5f53544f434ba974",
            "69636b5f73697a65b6302e30303120313020302e30303520353020302e3031af",
            "7469636b5f73697a655f72756c65739382a47469636ba5302e303031ab757070",
            "65725f626f756e64a2313082a47469636ba5302e303035ab75707065725f626f",
            "756e64a2353082a47469636ba4302e3031ab75707065725f626f756e64c0ab75",
            "70646174655f64617465aa323032362d30382d3139b46d657461646174615f6f",
            "627365727665645f6174be323032362d30382d31395432333a35393a35392e31",
            "32333435363738395a"
        )
    );
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
