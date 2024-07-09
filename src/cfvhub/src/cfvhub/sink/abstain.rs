use super::{SinkExt, FormaterExt};
use serde::Serialize;

#[derive(Debug, Default)]
pub struct DoNothingSink {
    // do nothing
}

impl<In: Serialize> SinkExt<In> for DoNothingSink {
    fn build(_id: &str) -> Self {
        Self {}
    }
    fn exec(&mut self, input: &In, formater: &impl FormaterExt<In>) {
        let _formated = formater.format(input);
    }
}

