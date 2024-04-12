use super::formater::{Formated, FormaterExt};
use serde::Serialize;
use std::fs::OpenOptions;
use std::io::prelude::Write;
use tracing::error;
pub trait SinkExt<In>
where
    In: Serialize,
{
    // type In;
    // type F;
    fn exec(&mut self, input: &In, formater: &impl FormaterExt<In>);
    // fn format(&self, input: &Self::In) -> Self::F;
}

pub struct DiskSink {
    path: std::path::PathBuf,
    file: std::fs::File,
}

impl DiskSink {
    pub fn new(path: std::path::PathBuf) -> Self {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open(path.clone())
            .unwrap();
        Self { path, file }
    }
}

impl<In: Serialize> SinkExt<In> for DiskSink {
    fn exec(&mut self, input: &In, formater: &impl FormaterExt<In>) {
        let formated = formater.format(input);

        match formated {
            Ok(formated) => match formated {
                Formated::String(s) => {
                    if let Err(e) = writeln!(self.file, "{}", s) {
                        error!("write event to file error: {}", e)
                    }
                }
                Formated::Bytes(b) => {
                    if let Err(e) = self.file.write_all(&b) {
                        error!("write event bytes to file error: {}", e)
                    }
                }
            },
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }
}
