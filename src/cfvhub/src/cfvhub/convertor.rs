use cfapi::binding::MessageEvent;
use cfapi::event_reader::{EventReader, EventReaderSerConfig};
use cfapi::value::CFValue;
use serde::Serialize;
use std::collections::BTreeMap;

// Stateless
// Stateful
pub trait Convertor {
    type Out: Serialize;

    fn convert(&self, event: &MessageEvent) -> Self::Out;
}

// pub trait Convertor<Out> {
//     fn convert(&self, event: &MessageEvent) -> Out;
// }

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

    fn convert(&self, event: &MessageEvent) -> Self::Out {
        let mut reader = EventReader::new(event, &self.reader_config);
        reader.to_map()
    }
}
