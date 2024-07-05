use super::{DiskSinkPathSnafu, DiskSinkReadFileSnafu, Formated, FormaterExt, SinkError, SinkExt};
use serde::Serialize;
use snafu::ResultExt;
use std::fs::OpenOptions;
use std::io::prelude::Write;
use std::str::FromStr;
use tracing::error;

#[derive(Debug)]
pub struct DiskSink {
    pub path: std::path::PathBuf,
    file: std::fs::File,
    // file: Arc<Mutex<std::fs::File>>,
}

// pub struct DiskSinkConfig {
//     pub path: String,
// }

impl Default for DiskSink {
    fn default() -> Self {
        let path = dotenvy::var("DISK_SINK_PATH").unwrap_or_else(|_| "disk_sink.log".to_string());
        Self::new(&path).unwrap()
    }
}

impl DiskSink {
    pub fn new(path: &str) -> Result<Self, SinkError> {
        let path = std::path::PathBuf::from_str(path).context(DiskSinkPathSnafu)?;
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open(path.clone())
            .context(DiskSinkReadFileSnafu)?;
        // Ok(Self { path, file: Arc::new(Mutex::new(file)) })
        // Ok(Self { path, file})
        Ok(Self { path, file })
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
