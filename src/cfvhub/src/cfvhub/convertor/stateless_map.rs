use cfapi::binding::MessageEvent;
use cfapi::event_reader::{EventReader, EventReaderSerConfig};
use cfapi::value::CFValue;

use super::Convertor;
use std::collections::BTreeMap;

pub struct BTreeMapConvertor {
    reader_config: EventReaderSerConfig,
}

impl Default for BTreeMapConvertor {
    fn default() -> Self {
        Self {
            reader_config: EventReaderSerConfig::default()
                .with_event_type(true)
                .with_src(true),
        }
    }
}

impl BTreeMapConvertor {
    pub fn new(reader_config: EventReaderSerConfig) -> Self {
        Self { reader_config }
    }
}

impl Convertor for BTreeMapConvertor {
    type Out = BTreeMap<String, CFValue>;

    fn convert(&self, event: &MessageEvent) -> Option<Self::Out> {
        let mut reader = EventReader::new(event, &self.reader_config);
        Some(reader.to_map())
    }
}
