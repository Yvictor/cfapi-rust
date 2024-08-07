
use ahash::RandomState;
use cfapi::binding::MessageEvent;
use cfapi::event_reader::{EventReader, EventReaderSerConfig};
use cfapi::value::CFValue;
use dashmap::DashMap;

use std::collections::BTreeMap;
use tracing::{debug, info};
use super::Convertor;
use itertools::Itertools;



pub struct StatefulBTreeMapConvertor {
    reader_config: EventReaderSerConfig,
    state: DashMap<String, BTreeMap<String, CFValue>, RandomState>,
}

impl StatefulBTreeMapConvertor {
    pub fn new(reader_config: EventReaderSerConfig) -> Self {
        Self {
            reader_config,
            state: DashMap::with_hasher(RandomState::new()),
        }
    }
}

impl Default for StatefulBTreeMapConvertor {
    fn default() -> Self {
        Self::new(
            EventReaderSerConfig::default()
                .with_event_type(true)
                .with_src(true),
        )
    }
}

impl Convertor for StatefulBTreeMapConvertor {
    type Out = BTreeMap<String, CFValue>;

    fn convert(&self, event: &MessageEvent) -> Option<Self::Out> {
        let src = i32::from(event.getSource());
        let symbol = event.getSymbol();
        let key = format!("{}.{}", src, symbol);
        // maybe consider event type for update or create
        let event_type = event.getType();
        // debug!("key: {}, {}", key, event_type);
        // let status_code = i32::from(event.getStatusCode());
        // let tag = event.getTag();
        // debug!("event type: {:?}, status code: {}, tag: {}", event_type, (status_code), tag);
        let mut reader = EventReader::new(event, &self.reader_config);
        let map = reader.to_map();
        let updated_keys = map.keys().join(",");
        debug!("key: {}, {}, keys: {}", key, event_type, updated_keys);
        let updated_map = match self.state.get_mut(&key) {
            Some(mut state) => {
                state.extend(map);
                state.clone()
            }
            None => {
                self.state.insert(key, map.clone());
                map
            }
        };
        Some(updated_map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cfapi::value::CFValue;
    use std::collections::BTreeMap;

    #[test]
    fn test_dashmap_usage() {
        let state = DashMap::with_hasher(RandomState::new());
        let mut map = BTreeMap::new();
        map.insert("abc", CFValue::Double(0.5));
        map.insert("vvv", CFValue::Int(1));
        state.insert("src.symbol", map);
        let mut new_map = BTreeMap::new();
        new_map.insert("new", CFValue::String("value".into()));
        // let vaule = state.get_mut("symbol.src");
        let new_v = match state.get_mut("src.symbol") {
            Some(mut v) => {
                v.extend(new_map);
                v.clone()
            }
            None => {
                println!("src.symbol not found");
                new_map
            }
        };
        print!("state: {:?}", state);
        println!("{:?}", new_v);
        let new_v = state.get("src.symbol");
        match new_v {
            Some(v) => {
                let new_field = v.get("new");
                match new_field {
                    Some(f) => match *f {
                        CFValue::String(ref s) => {
                            assert_eq!(s, "value");
                        }
                        _ => {
                            assert!("cf value not string." == "");
                        }
                    },
                    None => {
                        assert!("new field not exist." == "");
                    }
                }
            }
            None => {
                assert!(false);
            }
        }
    }
}
