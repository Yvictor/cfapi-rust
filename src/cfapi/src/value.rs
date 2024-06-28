use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CFValue {
    String(String),
    Double(f64),
    Int(i64),
    Datetime(f64),
    Unknown,
}


impl CFValue {
    pub fn to_i64(self) -> i64 {
        match self {
            CFValue::Int(v) => v,
            _ => 0,
        }
    }

    pub fn to_f64(self) -> f64 {
        match self {
            CFValue::Double(v) => v,
            CFValue::Datetime(v) => v,
            _ => 0.0,
        }
    }

    pub fn to_string(self) -> String {
        match self {
            CFValue::String(v) => v,
            _ => "".to_string(),
        }
    }
}