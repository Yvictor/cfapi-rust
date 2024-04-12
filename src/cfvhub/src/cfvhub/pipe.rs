use super::formater::FormaterExt;
use super::sink::SinkExt;
use cfapi::binding::MessageEvent;

use cfapi::message_event::MessageEventHandlerExt;

use super::convertor::Convertor;

pub struct PipeMessageHandler<C, F, R>
where
    C: Convertor,
    F: FormaterExt<C::Out>,
    R: SinkExt<C::Out>,
{
    convertor: C,
    formater: F,
    sink: R,
}

impl<C, F, R> PipeMessageHandler<C, F, R>
where
    C: Convertor,
    F: FormaterExt<C::Out>,
    R: SinkExt<C::Out>,
{
    pub fn new(convertor: C, formater: F, sink: R) -> Self {
        Self {
            convertor,
            formater,
            sink,
        }
    }
}

impl<C, F, R> MessageEventHandlerExt for PipeMessageHandler<C, F, R>
where
    C: Convertor,
    F: FormaterExt<C::Out>,
    R: SinkExt<C::Out>,
{
    fn on_message_event(&mut self, event: &MessageEvent) {
        self.sink.exec(&self.convertor.convert(event), &self.formater);
    }
}
