use super::formater::FormaterExt;
use super::sink::SinkExt;
use cfapi::binding::MessageEvent;
use std::sync::Mutex;

use cfapi::message_event::MessageEventHandlerExt;
use tracing::info;

use super::convertor::Convertor;

pub struct PipeMessageHandler<C, F, R>
where
    C: Convertor + Send + Sync,
    F: FormaterExt<C::Out> + Send + Sync,
    R: SinkExt<C::Out> + Send,
{
    convertor: C,
    formater: F,
    sink: Mutex<R>,
}

impl<C, F, R> PipeMessageHandler<C, F, R>
where
    C: Convertor + Send + Sync,
    F: FormaterExt<C::Out> + Send + Sync,
    R: SinkExt<C::Out> + Send,
{
    pub fn new(convertor: C, formater: F, sink: R) -> Self {
        Self {
            convertor,
            formater,
            sink: Mutex::new(sink),
        }
    }
}

impl<C, F, R> MessageEventHandlerExt for PipeMessageHandler<C, F, R>
where
    C: Convertor + Send + Sync,
    F: FormaterExt<C::Out> + Send + Sync,
    R: SinkExt<C::Out> + Send,
{
    fn on_message_event(&self, event: &MessageEvent) {
        // let src = i32::from(event.getSource());
        // let symbol = event.getSymbol();
        // let key = format!("{}.{}", src, symbol);
        // info!("key: {}", key);
        if event.getSource() == autocxx::c_int(0) {
            return;
        }
        let data = self.convertor.convert(event);
        match data {
            Some(data) => {
                self.sink.lock().unwrap().exec(&data, &self.formater);
            }
            None => {}
        }
        // std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
