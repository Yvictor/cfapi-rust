use super::formater::{Formated, FormaterExt};
use serde::Serialize;
use snafu::prelude::Snafu;
use std::io;
// use std::sync::{Arc, Mutex};

#[derive(Debug, Snafu)]
pub enum SinkError {
    #[snafu(display("DiskSink Readfile Error: {}", source))]
    DiskSinkReadFile { source: io::Error },
    #[snafu(display("DiskSink Path Error: {}", source))]
    DiskSinkPath { source: std::convert::Infallible },
}

pub trait SinkExt<In>
where
    In: Serialize,
{
    // type In;
    // type F;
    fn exec(&mut self, input: &In, formater: &impl FormaterExt<In>);
    // fn format(&self, input: &Self::In) -> Self::F;
}

pub trait Dest {
    fn get_dest(&self) -> &str;
}

pub mod abstain;
pub mod console;
pub mod disk;
pub mod solace;
pub use abstain::DoNothingSink;
pub use console::ConsoleSink;
pub use disk::DiskSink;
pub use solace::SolaceSink;
