use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CFValue {
    String(String),
    Double(f64),
    Int(i64),
    Datetime(f64),
    Unknown,
}
