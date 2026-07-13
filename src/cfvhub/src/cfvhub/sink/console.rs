use super::{ByteSink, Formated, FormaterExt, SinkExt};
use serde::Serialize;
use std::io::Write;

#[derive(Debug, Default)]
pub struct ConsoleSink {}

impl<In: Serialize> SinkExt<In> for ConsoleSink {
    fn build(_id: &str) -> Self {
        Self {}
    }

    fn exec(&mut self, input: &In, formater: &impl FormaterExt<In>) {
        let formated = formater.format(input);

        match formated {
            Ok(formated) => match formated {
                Formated::String(s) => {
                    println!("{}", s);
                }
                Formated::Bytes(b) => {
                    println!("{:?}", b);
                }
            },
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct SolaceConsoleSink {
    id: String,
    enabled: bool,
}

impl<In> SinkExt<In> for SolaceConsoleSink
where
    In: Serialize + super::Dest + std::fmt::Debug,
{
    fn build(id: &str) -> Self {
        let enabled = dotenvy::var("CFVHUB_SINK")
            .map(|value| {
                !matches!(
                    value.to_ascii_lowercase().as_str(),
                    "none" | "noop" | "null" | "off" | "0"
                )
            })
            .unwrap_or(true);
        Self {
            id: id.to_string(),
            enabled,
        }
    }

    fn exec(&mut self, input: &In, formater: &impl FormaterExt<In>) {
        if !self.enabled {
            return;
        }
        let topic = input.get_dest();
        let content_type = formater.content_type();
        match formater.format(input) {
            Ok(Formated::String(body)) => {
                write_console_line(format_args!(
                    "[solace-console:{}] topic={} content_type={} payload={:?} body={}",
                    self.id, topic, content_type, input, body
                ));
            }
            Ok(Formated::Bytes(bytes)) => {
                let preview_len = bytes.len().min(32);
                let preview = bytes[..preview_len]
                    .iter()
                    .map(|byte| format!("{:02x}", byte))
                    .collect::<Vec<_>>()
                    .join("");
                let json_preview = serde_json::to_string(input)
                    .unwrap_or_else(|error| format!("<json encode error: {}>", error));
                write_console_line(format_args!(
                    "[solace-console:{}] topic={} content_type={} msgpack_len={} msgpack_hex_prefix={} payload={:?} json={}",
                    self.id,
                    topic,
                    content_type,
                    bytes.len(),
                    preview,
                    input,
                    json_preview
                ));
            }
            Err(error) => {
                eprintln!(
                    "[solace-console:{}] topic={} format error: {}",
                    self.id, topic, error
                );
            }
        }
    }
}

impl ByteSink for SolaceConsoleSink {
    fn build(id: &str) -> Self {
        let enabled = dotenvy::var("CFVHUB_SINK")
            .map(|value| {
                !matches!(
                    value.to_ascii_lowercase().as_str(),
                    "none" | "noop" | "null" | "off" | "0"
                )
            })
            .unwrap_or(true);
        Self {
            id: id.to_string(),
            enabled,
        }
    }

    fn exec_bytes(&mut self, destination: &str, content_type: &str, payload: &[u8]) -> bool {
        if self.enabled {
            let preview_len = payload.len().min(32);
            let preview = payload[..preview_len]
                .iter()
                .map(|byte| format!("{:02x}", byte))
                .collect::<Vec<_>>()
                .join("");
            write_console_line(format_args!(
                "[solace-console:{}] topic={} content_type={} msgpack_len={} msgpack_hex_prefix={}",
                self.id,
                destination,
                content_type,
                payload.len(),
                preview
            ));
        }
        true
    }
}

fn write_console_line(args: std::fmt::Arguments<'_>) {
    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_fmt(args);
    let _ = stdout.write_all(b"\n");
}
