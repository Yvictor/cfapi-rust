use std::{
    any::Any,
    cell::RefCell,
    collections::HashMap,
    fmt::Debug,
    marker::PhantomData,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
    time::Instant,
};

use super::convertor::Convertor;
use super::formater::FormaterExt;
use super::sink::SinkExt;
use cfapi::binding::MessageEvent;
use cfapi::message_event::MessageEventHandlerExt;
use tracing::{error, info};

static NEXT_HANDLER_ID: AtomicUsize = AtomicUsize::new(1);

thread_local! {
    static THREAD_SHARDS: RefCell<HashMap<usize, Box<dyn Any>>> = RefCell::new(HashMap::new());
}

struct ThreadShard<F, R> {
    formater: F,
    sink: R,
}

pub struct PipeThreadLocalMessageHandler<C, F, R>
where
    C: Convertor + Send + Sync,
    F: FormaterExt<C::Out> + Default + 'static,
    R: SinkExt<C::Out> + 'static,
    C::Out: Debug,
{
    id: usize,
    convertor: C,
    handled_count: Arc<AtomicU64>,
    handled_nanos: Arc<AtomicU64>,
    _types: PhantomData<fn() -> (F, R)>,
}

impl<C, F, R> PipeThreadLocalMessageHandler<C, F, R>
where
    C: Convertor + Send + Sync,
    F: FormaterExt<C::Out> + Default + 'static,
    R: SinkExt<C::Out> + 'static,
    C::Out: Debug,
{
    pub fn new(convertor: C) -> Self {
        Self {
            id: NEXT_HANDLER_ID.fetch_add(1, Ordering::Relaxed),
            convertor,
            handled_count: Arc::new(AtomicU64::new(0)),
            handled_nanos: Arc::new(AtomicU64::new(0)),
            _types: PhantomData,
        }
    }

    pub fn exec_metrics_loop_th(&self) {
        let handled_count = Arc::clone(&self.handled_count);
        let handled_nanos = Arc::clone(&self.handled_nanos);
        std::thread::spawn(move || {
            let mut last_count = 0;
            let mut last_nanos = 0;
            loop {
                std::thread::sleep(std::time::Duration::from_secs(10));
                let count = handled_count.load(Ordering::Relaxed);
                let nanos = handled_nanos.load(Ordering::Relaxed);
                let delta_count = count.saturating_sub(last_count);
                let delta_nanos = nanos.saturating_sub(last_nanos);
                let avg_callback_us = if delta_count == 0 {
                    0.0
                } else {
                    delta_nanos as f64 / delta_count as f64 / 1_000.0
                };
                info!(
                    total_handled = count,
                    handled_per_sec = delta_count as f64 / 10.0,
                    avg_callback_us,
                    "thread-local pipe metrics"
                );
                last_count = count;
                last_nanos = nanos;
            }
        });
    }
}

impl<C, F, R> MessageEventHandlerExt for PipeThreadLocalMessageHandler<C, F, R>
where
    C: Convertor + Send + Sync,
    F: FormaterExt<C::Out> + Default + 'static,
    R: SinkExt<C::Out> + 'static,
    C::Out: Debug,
{
    fn on_message_event(&self, event: &MessageEvent) {
        if event.getSource() == autocxx::c_int(0) {
            return;
        }

        let started = Instant::now();
        let Some(data) = self.convertor.convert(event) else {
            return;
        };

        THREAD_SHARDS.with(|shards| {
            let mut shards = shards.borrow_mut();
            let shard = shards.entry(self.id).or_insert_with(|| {
                let id = format!("{:?}", std::thread::current().id());
                info!(
                    thread_id = id,
                    handler_id = self.id,
                    "building thread-local sink"
                );
                Box::new(ThreadShard::<F, R> {
                    formater: F::default(),
                    sink: R::build(&id),
                })
            });

            let Some(shard) = shard.downcast_mut::<ThreadShard<F, R>>() else {
                error!(handler_id = self.id, "thread-local shard type mismatch");
                return;
            };
            shard.sink.exec(&data, &shard.formater);
        });

        self.handled_count.fetch_add(1, Ordering::Relaxed);
        self.handled_nanos.fetch_add(
            started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64,
            Ordering::Relaxed,
        );
    }
}
