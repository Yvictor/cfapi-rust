use super::{SinkExt, FormaterExt, Formated};
use serde::Serialize;

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
