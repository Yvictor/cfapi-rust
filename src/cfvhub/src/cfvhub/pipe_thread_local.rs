use std::{
    any::Any,
    cell::RefCell,
    collections::{BTreeSet, HashMap},
    fmt::Debug,
    marker::PhantomData,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex,
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
    callback_threads: Arc<Mutex<BTreeSet<String>>>,
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
            callback_threads: Arc::new(Mutex::new(BTreeSet::new())),
            _types: PhantomData,
        }
    }

    pub fn exec_metrics_loop_th(&self) {
        let handled_count = Arc::clone(&self.handled_count);
        let handled_nanos = Arc::clone(&self.handled_nanos);
        let callback_threads = Arc::clone(&self.callback_threads);
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
                let thread_ids = callback_threads
                    .lock()
                    .map(|threads| threads.iter().cloned().collect::<Vec<_>>())
                    .unwrap_or_default();
                info!(
                    total_handled = count,
                    handled_per_sec = delta_count as f64 / 10.0,
                    avg_callback_us,
                    callback_threads_seen = thread_ids.len(),
                    callback_thread_ids = ?thread_ids,
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
        let thread_id = format!("{:?}", std::thread::current().id());
        self.record_callback_thread(&thread_id);

        THREAD_SHARDS.with(|shards| {
            let mut shards = shards.borrow_mut();
            let shard = shards.entry(self.id).or_insert_with(|| {
                info!(
                    thread_id = thread_id.as_str(),
                    handler_id = self.id,
                    "building thread-local sink"
                );
                Box::new(ThreadShard::<F, R> {
                    formater: F::default(),
                    sink: R::build(&thread_id),
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

impl<C, F, R> PipeThreadLocalMessageHandler<C, F, R>
where
    C: Convertor + Send + Sync,
    F: FormaterExt<C::Out> + Default + 'static,
    R: SinkExt<C::Out> + 'static,
    C::Out: Debug,
{
    fn record_callback_thread(&self, thread_id: &str) {
        let mut threads = self.callback_threads.lock().unwrap();
        if threads.insert(thread_id.to_owned()) {
            info!(
                thread_id,
                handler_id = self.id,
                callback_threads_seen = threads.len(),
                "new callback thread"
            );
        }
    }
}
