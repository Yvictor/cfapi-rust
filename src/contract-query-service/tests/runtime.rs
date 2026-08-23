#![cfg(feature = "runtime")]

use contract_query_service::runtime::{ConfigError, RuntimeConfig};
use std::collections::HashMap;

fn required() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("CFAPI_USERNAME", "runtime-user"),
        ("CFAPI_PASSWORD", "highly-sensitive-password"),
        ("CFAPI_HOSTS", "216.221.209.61:7022,216.221.209.62:7022"),
    ])
}

#[test]
fn parses_defaults_without_exposing_credentials() {
    let config = RuntimeConfig::from_values(required()).expect("valid config");
    assert_eq!(config.bind().to_string(), "0.0.0.0:8080");
    assert_eq!(config.sources(), &[533, 534]);
    assert_eq!(config.max_user_threads(), 0);
    assert_eq!(config.max_csp_threads(), 32);
    assert!(config.auto_sync());
    assert_eq!(config.hosts().len(), 2);
}

#[test]
fn requires_password_and_never_echoes_it() {
    let mut values = required();
    values.remove("CFAPI_PASSWORD");
    let error = RuntimeConfig::from_values(values).err().expect("must fail");
    assert_eq!(error, ConfigError::Missing("CFAPI_PASSWORD"));
    assert!(!error.to_string().contains("runtime-user"));
}

#[test]
fn accepts_legacy_cfapi_credential_and_host_aliases() {
    let values = HashMap::from([
        ("CFAPI_USER", "legacy-user"),
        ("CFAPI_PASS", "legacy-password"),
        ("CFAPI_HOST", "216.221.209.61:7022"),
    ]);
    let config = RuntimeConfig::from_values(values).expect("legacy aliases are supported");
    assert_eq!(config.hosts(), &["216.221.209.61:7022"]);
}

#[test]
fn primary_cfapi_names_take_priority_over_aliases() {
    let mut values = required();
    values.extend([
        ("CFAPI_USER", "ignored-user"),
        ("CFAPI_PASS", "ignored-password"),
        ("CFAPI_HOST", "invalid-ignored-host"),
    ]);
    let config = RuntimeConfig::from_values(values).expect("primary values win");
    assert_eq!(config.hosts().len(), 2);
}

#[test]
fn invalid_primary_value_does_not_fall_back_to_alias() {
    let mut values = required();
    values.insert("CFAPI_PASSWORD", "");
    values.insert("CFAPI_PASS", "fallback-must-not-be-used");
    assert_eq!(
        RuntimeConfig::from_values(values).err(),
        Some(ConfigError::Missing("CFAPI_PASSWORD"))
    );
}

#[test]
fn errors_name_keys_not_secret_values() {
    let mut values = required();
    values.insert("CONTRACT_QUERY_TIMEOUT_MS", "highly-sensitive-password");
    let error = RuntimeConfig::from_values(values).err().expect("must fail");
    assert_eq!(error, ConfigError::Invalid("CONTRACT_QUERY_TIMEOUT_MS"));
    assert!(!error.to_string().contains("highly-sensitive-password"));
}

#[test]
fn parses_explicit_runtime_topology() {
    let mut values = required();
    values.extend([
        ("CONTRACT_HTTP_BIND", "127.0.0.1:9080"),
        ("CFAPI_SOURCES", "534,533,534"),
        ("CFAPI_MAX_USER_THREADS", "2"),
        ("CFAPI_MAX_CSP_THREADS", "16"),
        ("CONTRACT_AUTO_SYNC", "false"),
    ]);
    let config = RuntimeConfig::from_values(values).expect("valid config");
    assert_eq!(config.bind().to_string(), "127.0.0.1:9080");
    assert_eq!(config.sources(), &[533, 534]);
    assert_eq!(config.max_user_threads(), 2);
    assert_eq!(config.max_csp_threads(), 16);
    assert!(!config.auto_sync());
}

#[test]
fn validates_limits_before_constructing_cfapi() {
    for (key, value, expected) in [
        (
            "CFAPI_COMMAND_CAPACITY",
            "0",
            ConfigError::OutOfRange("CFAPI_COMMAND_CAPACITY"),
        ),
        (
            "CFAPI_MAX_REQUEST_QUEUE_SIZE",
            "99999",
            ConfigError::OutOfRange("CFAPI_MAX_REQUEST_QUEUE_SIZE"),
        ),
        (
            "CONTRACT_ITEM_QUEUE_CAPACITY",
            "0",
            ConfigError::OutOfRange("CONTRACT_ITEM_QUEUE_CAPACITY"),
        ),
    ] {
        let mut values = required();
        values.insert(key, value);
        assert_eq!(RuntimeConfig::from_values(values).err(), Some(expected));
    }
}

#[test]
fn validates_cfapi_host_port_without_echoing_the_value() {
    let mut values = required();
    values.insert("CFAPI_HOSTS", "highly-sensitive-password");
    let error = RuntimeConfig::from_values(values).err().expect("must fail");
    assert_eq!(error, ConfigError::Invalid("CFAPI_HOSTS"));
    assert!(!error.to_string().contains("highly-sensitive-password"));
}

#[test]
fn validates_source_and_concurrency_relationships() {
    let mut values = required();
    values.insert("CFAPI_SOURCES", "533,534,535");
    assert_eq!(
        RuntimeConfig::from_values(values).err(),
        Some(ConfigError::OutOfRange("CONTRACT_MAX_WHOLE_SOURCE_QUERIES"))
    );

    let mut values = required();
    values.insert("CONTRACT_MAX_EXACT_QUERIES", "8");
    values.insert("CONTRACT_MAX_EXACT_PER_SOURCE", "9");
    assert_eq!(
        RuntimeConfig::from_values(values).err(),
        Some(ConfigError::OutOfRange("CONTRACT_MAX_EXACT_PER_SOURCE"))
    );
}
