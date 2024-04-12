use super::value::CFValue;
use super::binding::{GetEventReader, MessageEvent, MessageEvent_Types, MessageReader, ValueTypes};
use std::collections::BTreeMap;

pub struct EventReaderSerConfig {
    with_event_type: bool,
    with_src: bool,
}

impl Default for EventReaderSerConfig {
    fn default() -> Self {
        EventReaderSerConfig {
            with_event_type: false,
            with_src: false,
        }
    }
}

impl EventReaderSerConfig {
    pub fn with_event_type(mut self, with_event_type: bool) -> Self {
        self.with_event_type = with_event_type;
        self
    }

    pub fn with_src(mut self, with_src: bool) -> Self {
        self.with_src = with_src;
        self
    }
}

pub struct EventReader<'a> {
    event: &'a MessageEvent,
    reader: std::pin::Pin<&'a mut MessageReader>,
    ser_config: &'a EventReaderSerConfig,
}

impl<'a> EventReader<'a> {
    pub fn new(event: &'a MessageEvent, ser_config: &'a EventReaderSerConfig) -> Self {
        let reader = GetEventReader(event) as *mut MessageReader;
        let reader = unsafe { std::pin::Pin::new_unchecked(&mut *reader) };
        EventReader { event, reader, ser_config}
    }
    pub fn with_ser_config(mut self, ser_config: &'a EventReaderSerConfig) -> Self {
        self.ser_config = ser_config;
        self
    }
}

impl EventReader<'_> {
    pub fn to_map(&mut self) -> BTreeMap<String, CFValue> {
        let mut map = BTreeMap::new();
        if self.ser_config.with_event_type {
            let event_type = self.event.getType() as MessageEvent_Types;
            let event_type = match event_type {
                MessageEvent_Types::IMAGE_COMPLETE => "IMAGE_COMPLETE",
                MessageEvent_Types::IMAGE_PART => "IMAGE_PART",
                MessageEvent_Types::REFRESH => "REFRESH",
                MessageEvent_Types::STATUS => "STATUS",
                MessageEvent_Types::UPDATE => "UPDATE",
            };
            map.insert(
                "(0)EventType".to_owned(),
                CFValue::String(event_type.to_owned()),
            );
        }
        if self.ser_config.with_src {
            map.insert(
                "(1)Source".to_owned(),
                CFValue::Int(i32::from(self.event.getSource()) as i64),
            );
        }
        let symbol = self.event.getSymbol();
        map.insert("(2)Symbol".to_owned(), CFValue::String(symbol.to_string()));
        while self.reader.as_mut().next() != autocxx::c_int(-1) {
            let key = format!(
                "({}){}",
                i32::from(self.reader.as_mut().getTokenNumber()),
                self.reader.as_mut().getTokenName()
            );
            let value = match self.reader.as_mut().getValueType() {
                ValueTypes::INT64 => CFValue::Int(self.reader.as_mut().getValueAsInteger()),
                ValueTypes::DOUBLE => CFValue::Double(self.reader.as_mut().getValueAsDouble()),
                ValueTypes::STRING => {
                    CFValue::String(self.reader.as_mut().getValueAsString().to_string())
                }
                ValueTypes::DATETIME => CFValue::Datetime(self.reader.as_mut().getValueAsDouble()),
                ValueTypes::UNKNOWN => CFValue::Unknown,
            };
            map.insert(key, value);
        }
        map
    }

    pub fn to_json(&mut self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&mut self.to_map())
    }

    pub fn to_msgpack(&mut self) -> Result<Vec<u8>, rmp_serde::encode::Error> {
        rmp_serde::to_vec_named(&self.to_map())
    }
}
