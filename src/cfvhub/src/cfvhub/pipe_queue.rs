use std::{
    fmt::Debug,
    marker::PhantomData,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Instant,
};

use super::convertor::Convertor;
use super::formater::FormaterExt;
use super::sink::SinkExt;
use cfapi::binding::MessageEvent;
use cfapi::message_event::MessageEventHandlerExt;
use crossbeam_channel::{bounded, Receiver, Sender};
use minitrace::prelude::{LocalSpan, Span, SpanContext};
use tracing::{error, info, warn};

pub struct PipeQueueMessageHandler<C, F, R>
where
    C: Convertor + Send + Sync,
    F: FormaterExt<C::Out> + Send + Sync,
    R: SinkExt<C::Out> + Send + Sync,
    C::Out: Send + Sync + Debug,
{
    convertor: C,
    worker_senders: Vec<Sender<C::Out>>,
    worker_receivers: Vec<Receiver<C::Out>>,
    routed_count: Arc<AtomicU64>,
    routed_nanos: Arc<AtomicU64>,
    _formater: PhantomData<F>,
    _sink: PhantomData<R>,
}

impl<C, F, R> PipeQueueMessageHandler<C, F, R>
where
    C: Convertor + Send + Sync,
    F: FormaterExt<C::Out> + Send + Sync + Default,
    R: SinkExt<C::Out> + Send + Sync + Default,
    C::Out: Send + Sync + Debug,
{
    pub fn new(convertor: C, size: usize, n: usize) -> Self
    where
        F: FormaterExt<C::Out> + Send + Sync + Default,
        R: SinkExt<C::Out> + Send + Sync + Default,
    {
        let worker_count = n.max(1);
        let mut worker_senders = Vec::with_capacity(worker_count);
        let mut worker_receivers = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let (send, recv) = bounded(size);
            worker_senders.push(send);
            worker_receivers.push(recv);
        }

        Self {
            convertor,
            worker_senders,
            worker_receivers,
            routed_count: Arc::new(AtomicU64::new(0)),
            routed_nanos: Arc::new(AtomicU64::new(0)),
            _formater: PhantomData,
            _sink: PhantomData,
        }
    }

    pub fn exec_loop_th(&self)
    where
        <C as Convertor>::Out: 'static,
        F: FormaterExt<C::Out> + Send + Sync + Default,
        R: SinkExt<C::Out> + Send + Sync + Default,
    {
        for (i, recv) in self.worker_receivers.iter().cloned().enumerate() {
            let id = i.to_string();
            std::thread::spawn(move || {
                let formater = F::default();
                let mut sink = R::build(&id);
                loop {
                    match recv.recv() {
                        Ok(data) => sink.exec(&data, &formater),
                        Err(e) => {
                            error!(worker = i, "worker channel disconnected: {:?}", e);
                            break;
                        }
                    }
                }
            });
        }

        let receivers = self.worker_receivers.clone();
        let routed_count = Arc::clone(&self.routed_count);
        let routed_nanos = Arc::clone(&self.routed_nanos);
        std::thread::spawn(move || {
            let mut last_count = 0;
            let mut last_nanos = 0;
            loop {
                std::thread::sleep(std::time::Duration::from_secs(10));
                let queue_sizes: Vec<usize> = receivers.iter().map(Receiver::len).collect();
                let count = routed_count.load(Ordering::Relaxed);
                let nanos = routed_nanos.load(Ordering::Relaxed);
                let delta_count = count.saturating_sub(last_count);
                let delta_nanos = nanos.saturating_sub(last_nanos);
                let avg_route_us = if delta_count == 0 {
                    0.0
                } else {
                    delta_nanos as f64 / delta_count as f64 / 1_000.0
                };
                info!(
                    ?queue_sizes,
                    total_routed = count,
                    routed_per_sec = delta_count as f64 / 10.0,
                    avg_route_us,
                    "pipe queue metrics"
                );
                last_count = count;
                last_nanos = nanos;
            }
        });
    }
}

impl<C, F, R> PipeQueueMessageHandler<C, F, R>
where
    C: Convertor + Send + Sync,
    F: FormaterExt<C::Out> + Send + Sync,
    R: SinkExt<C::Out> + Send + Sync,
    C::Out: Send + Sync + Debug,
{
    pub fn get_queue_size(&self) -> usize {
        self.worker_receivers.iter().map(Receiver::len).sum()
    }

    pub fn get_worker_queue_sizes(&self) -> Vec<usize> {
        self.worker_receivers.iter().map(Receiver::len).collect()
    }

    fn worker_index(symbol: &str, worker_count: usize) -> usize {
        let mut hash = 0xcbf29ce484222325u64;
        for byte in symbol.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        (hash as usize) % worker_count
    }
}

impl<C, F, R> MessageEventHandlerExt for PipeQueueMessageHandler<C, F, R>
where
    C: Convertor + Send + Sync,
    F: FormaterExt<C::Out> + Send + Sync,
    R: SinkExt<C::Out> + Send + Sync,
    C::Out: Send + Sync + Debug,
{
    fn on_message_event(&self, event: &MessageEvent) {
        if event.getSource() == autocxx::c_int(0) {
            return;
        }

        let worker_count = self.worker_senders.len();
        if worker_count == 0 {
            warn!("message event dropped because no workers are configured");
            return;
        }

        let route_started = Instant::now();
        let symbol = event.getSymbol().to_string();
        let worker_index = Self::worker_index(&symbol, worker_count);
        let root = Span::root("on_msg", SpanContext::random());
        let _guard = root.set_local_parent();
        let data = self.convertor.convert(event);

        if let Some(data) = data {
            let _g = LocalSpan::enter_with_local_parent("route_data");
            if let Err(e) = self.worker_senders[worker_index].send(data) {
                error!(
                    symbol,
                    worker = worker_index,
                    "worker channel send error: {:?}",
                    e
                );
            } else {
                self.routed_count.fetch_add(1, Ordering::Relaxed);
                self.routed_nanos.fetch_add(
                    route_started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64,
                    Ordering::Relaxed,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formater::{Formated, FormaterExt};
    use crate::sink::SinkExt;
    use cfapi::binding::MessageEvent;
    use serde::Serialize;

    struct NoopConvertor;

    impl Convertor for NoopConvertor {
        type Out = String;

        fn convert(&self, _event: &MessageEvent) -> Option<Self::Out> {
            None
        }
    }

    #[derive(Default)]
    struct NoopFormater;

    impl FormaterExt<String> for NoopFormater {
        fn format(&self, input: &String) -> Result<Formated, crate::formater::FormatError> {
            Ok(Formated::String(input.clone()))
        }

        fn content_type(&self) -> &str {
            "text/plain"
        }
    }

    #[derive(Default)]
    struct NoopSink;

    impl<In: Serialize> SinkExt<In> for NoopSink {
        fn exec(&mut self, _input: &In, _formater: &impl FormaterExt<In>) {}

        fn build(_id: &str) -> Self {
            Self
        }
    }

    #[test]
    fn routes_same_symbol_to_same_worker() {
        type Handler = PipeQueueMessageHandler<NoopConvertor, NoopFormater, NoopSink>;
        let worker = Handler::worker_index("AAPL", 8);
        for _ in 0..100 {
            assert_eq!(worker, Handler::worker_index("AAPL", 8));
        }
    }

    #[test]
    fn routes_symbols_across_workers() {
        type Handler = PipeQueueMessageHandler<NoopConvertor, NoopFormater, NoopSink>;
        let workers = ["AAPL", "NVDA", "MSFT", "NKE", "TSLA"]
            .into_iter()
            .map(|symbol| Handler::worker_index(symbol, 8))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(workers.len() > 1);
    }
}
