use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use ahash::RandomState;
use cfapi::binding::MessageEvent;
use cfapi::event_reader::{EventReader, EventReaderSerConfig};
use cfapi::message_event::MessageEventHandlerExt;
use cfapi::value::CFValue;
use kanal::{bounded, Receiver, Sender};
use tracing::{error, info, warn};

use super::convertor::nasdaq_solace::{MessageUpdate, NasdaqSolaceMessage, SymbolState};
use super::sink::{ByteSink, Dest};

const NASDAQ_SOURCES: &[i32] = &[533, 534];
const SNAPSHOT_EVENT: i32 = 1;
const UPDATE_EVENT: i32 = 2;
const MAX_SYMBOL_BYTES: usize = 32;
const CALLBACK_LATENCY_SAMPLE_EVERY: u64 = 64;
const CALLBACK_LATENCY_UPPER_NS: [u64; 8] =
    [1_000, 1_500, 2_000, 2_500, 3_000, 4_000, 5_000, 10_000];
const CALLBACK_LATENCY_LABELS: [&str; 9] = [
    "<1us", "1-1.5us", "1.5-2us", "2-2.5us", "2.5-3us", "3-4us", "4-5us", "5-10us", ">=10us",
];

pub const NASDAQ_FILTER_TOKENS: &[i32] = &[
    8, 9, 10, 11, 12, 13, 16, 20, 55, 316, 361, 362, 388, 394, 400, 447, 448, 460, 463, 474, 1021,
    1708, 1709, 2500, 5004,
];

fn token_value_is_needed(token: i32) -> bool {
    matches!(
        token,
        8 | 9
            | 10
            | 11
            | 12
            | 13
            | 16
            | 55
            | 316
            | 361
            | 362
            | 388
            | 394
            | 400
            | 447
            | 448
            | 460
            | 463
            | 474
            | 1708
            | 1709
            | 2500
    )
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct SymbolKey {
    len: u8,
    bytes: [u8; MAX_SYMBOL_BYTES],
}

impl SymbolKey {
    fn from_bytes(value: &[u8]) -> Option<Self> {
        if value.is_empty() || value.len() > MAX_SYMBOL_BYTES {
            return None;
        }
        let mut bytes = [0; MAX_SYMBOL_BYTES];
        bytes[..value.len()].copy_from_slice(value);
        Some(Self {
            len: value.len() as u8,
            bytes,
        })
    }

    fn first_byte(self) -> u8 {
        self.bytes[0].to_ascii_uppercase()
    }

    fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..usize::from(self.len)]).unwrap_or("")
    }
}

#[derive(Debug)]
struct OwnedNasdaqEvent {
    source: i32,
    event_type: i32,
    symbol: SymbolKey,
    update: MessageUpdate,
}

#[derive(Debug)]
struct PublishFrame {
    topic: String,
    payload: Vec<u8>,
}

#[derive(Default)]
struct PipelineMetrics {
    callbacks: AtomicU64,
    callback_nanos: AtomicU64,
    callback_latency_buckets: [AtomicU64; CALLBACK_LATENCY_LABELS.len()],
    ignored: AtomicU64,
    invalid_symbol: AtomicU64,
    worker_handled: AtomicU64,
    encoded: AtomicU64,
    encode_errors: AtomicU64,
    published: AtomicU64,
    publish_errors: AtomicU64,
}

struct PrefixRouter {
    table: [usize; 256],
}

impl PrefixRouter {
    fn new(worker_count: usize) -> Self {
        let worker_count = worker_count.max(1);
        let mut table = [0; 256];
        for byte in 0u8..=u8::MAX {
            table[usize::from(byte)] = match byte.to_ascii_uppercase() {
                b'A'..=b'Z' => usize::from(byte.to_ascii_uppercase() - b'A') % worker_count,
                _ => usize::from(byte) % worker_count,
            };
        }
        Self { table }
    }

    fn route(&self, symbol: SymbolKey) -> usize {
        self.table[usize::from(symbol.first_byte())]
    }
}

struct WorkerState {
    states: HashMap<(i32, SymbolKey), SymbolState, RandomState>,
}

impl WorkerState {
    fn new() -> Self {
        Self {
            states: HashMap::with_hasher(RandomState::new()),
        }
    }

    fn process(&mut self, event: OwnedNasdaqEvent) -> [Option<NasdaqSolaceMessage>; 2] {
        let is_tick = event.update.is_tick();
        let is_bidask = event.update.is_bidask();
        let state = self
            .states
            .entry((event.source, event.symbol))
            .or_insert_with(|| SymbolState::new(event.symbol.as_str().to_owned()));
        state.apply(&event.update);

        if event.event_type == SNAPSHOT_EVENT {
            return [None, None];
        }
        if event.event_type != UPDATE_EVENT {
            return [None, None];
        }

        let mut output = [None, None];
        if is_tick {
            state.serial_num = state.serial_num.saturating_add(1);
            output[0] = Some(NasdaqSolaceMessage::Tick(state.to_tick()));
        }
        if is_bidask {
            state.serial_num = state.serial_num.saturating_add(1);
            let index = usize::from(output[0].is_some());
            output[index] = Some(NasdaqSolaceMessage::BidAsk(state.to_bidask()));
        }
        output
    }
}

pub struct PipeShardedMessageHandler<R>
where
    R: ByteSink,
{
    reader_config: EventReaderSerConfig,
    router: PrefixRouter,
    worker_senders: Vec<Sender<OwnedNasdaqEvent>>,
    metrics: Arc<PipelineMetrics>,
    _sink: PhantomData<fn() -> R>,
}

impl<R> PipeShardedMessageHandler<R>
where
    R: ByteSink,
{
    pub fn new(
        worker_count: usize,
        publisher_count: usize,
        worker_queue_capacity: usize,
        publisher_queue_capacity: usize,
    ) -> Self {
        let worker_count = worker_count.max(1);
        let publisher_count = publisher_count.max(1);
        let metrics = Arc::new(PipelineMetrics::default());
        let worker_counts = Arc::new(
            (0..worker_count)
                .map(|_| AtomicU64::new(0))
                .collect::<Vec<_>>(),
        );

        let mut publisher_senders = Vec::with_capacity(publisher_count);
        let mut publisher_receivers = Vec::with_capacity(publisher_count);
        for _ in 0..publisher_count {
            let (sender, receiver) = bounded(publisher_queue_capacity.max(1));
            publisher_senders.push(sender);
            publisher_receivers.push(receiver);
        }

        for (publisher_id, receiver) in publisher_receivers.into_iter().enumerate() {
            let metrics = Arc::clone(&metrics);
            std::thread::Builder::new()
                .name(format!("solace-publisher-{publisher_id}"))
                .spawn(move || run_publisher::<R>(publisher_id, receiver, metrics))
                .expect("spawn publisher thread");
        }

        let mut worker_senders = Vec::with_capacity(worker_count);
        let mut worker_receivers = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let (sender, receiver) = bounded(worker_queue_capacity.max(1));
            worker_senders.push(sender);
            worker_receivers.push(receiver);
        }

        for (worker_id, receiver) in worker_receivers.into_iter().enumerate() {
            let publisher = publisher_senders[worker_id % publisher_count].clone();
            let metrics = Arc::clone(&metrics);
            let worker_counts = Arc::clone(&worker_counts);
            std::thread::Builder::new()
                .name(format!("nasdaq-worker-{worker_id}"))
                .spawn(move || run_worker(worker_id, receiver, publisher, metrics, worker_counts))
                .expect("spawn state worker thread");
        }

        spawn_metrics(
            worker_senders.clone(),
            publisher_senders.clone(),
            Arc::clone(&metrics),
            worker_counts,
        );

        info!(
            worker_count,
            publisher_count,
            worker_queue_capacity,
            publisher_queue_capacity,
            "sharded pipeline started"
        );

        Self {
            reader_config: EventReaderSerConfig::default(),
            router: PrefixRouter::new(worker_count),
            worker_senders,
            metrics,
            _sink: PhantomData,
        }
    }

    fn decode(&self, event: &MessageEvent) -> Option<OwnedNasdaqEvent> {
        let source = i32::from(event.getSource());
        if !NASDAQ_SOURCES.contains(&source) {
            return None;
        }
        let event_type = event.getType() as i32;
        if event_type != SNAPSHOT_EVENT && event_type != UPDATE_EVENT {
            return None;
        }
        let Some(symbol) = SymbolKey::from_bytes(event.getSymbol().as_bytes()) else {
            self.metrics.invalid_symbol.fetch_add(1, Ordering::Relaxed);
            return None;
        };
        let mut update = MessageUpdate::default();
        let mut reader = EventReader::new(event, &self.reader_config);
        while let Some(token) = reader.next_token_number() {
            if token == 20 || token == 1021 {
                update.apply(token, CFValue::Unknown);
            } else if token_value_is_needed(token) {
                update.apply(token, reader.get_value());
            }
        }
        if event_type == UPDATE_EVENT && !update.is_tick() && !update.is_bidask() {
            return None;
        }
        Some(OwnedNasdaqEvent {
            source,
            event_type,
            symbol,
            update,
        })
    }
}

impl<R> MessageEventHandlerExt for PipeShardedMessageHandler<R>
where
    R: ByteSink,
{
    fn on_message_event(&self, event: &MessageEvent) {
        let started = Instant::now();
        let Some(owned) = self.decode(event) else {
            self.metrics.ignored.fetch_add(1, Ordering::Relaxed);
            return;
        };
        let worker = self.router.route(owned.symbol);
        if let Err(error) = self.worker_senders[worker].send(owned) {
            error!(worker, ?error, "state worker queue closed");
            return;
        }
        let elapsed_nanos = started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        let callback_index = self.metrics.callbacks.fetch_add(1, Ordering::Relaxed);
        self.metrics
            .callback_nanos
            .fetch_add(elapsed_nanos, Ordering::Relaxed);
        if callback_index % CALLBACK_LATENCY_SAMPLE_EVERY == 0 {
            let bucket = CALLBACK_LATENCY_UPPER_NS.partition_point(|upper| elapsed_nanos >= *upper);
            self.metrics.callback_latency_buckets[bucket].fetch_add(1, Ordering::Relaxed);
        }
    }
}

fn run_worker(
    worker_id: usize,
    receiver: Receiver<OwnedNasdaqEvent>,
    publisher: Sender<PublishFrame>,
    metrics: Arc<PipelineMetrics>,
    worker_counts: Arc<Vec<AtomicU64>>,
) {
    let mut state = WorkerState::new();
    while let Ok(event) = receiver.recv() {
        for message in state.process(event).into_iter().flatten() {
            let topic = message.get_dest().to_owned();
            match rmp_serde::to_vec(&message) {
                Ok(payload) => {
                    metrics.encoded.fetch_add(1, Ordering::Relaxed);
                    if let Err(error) = publisher.send(PublishFrame { topic, payload }) {
                        error!(worker_id, ?error, "publisher queue closed");
                        return;
                    }
                }
                Err(error) => {
                    metrics.encode_errors.fetch_add(1, Ordering::Relaxed);
                    error!(worker_id, ?error, "msgpack encode failed");
                }
            }
        }
        metrics.worker_handled.fetch_add(1, Ordering::Relaxed);
        worker_counts[worker_id].fetch_add(1, Ordering::Relaxed);
    }
    warn!(worker_id, "state worker stopped because its queue closed");
}

fn run_publisher<R>(
    publisher_id: usize,
    receiver: Receiver<PublishFrame>,
    metrics: Arc<PipelineMetrics>,
) where
    R: ByteSink,
{
    let mut sink = R::build(&format!("publisher-{publisher_id}"));
    while let Ok(frame) = receiver.recv() {
        if sink.exec_bytes(&frame.topic, "msgpack", &frame.payload) {
            metrics.published.fetch_add(1, Ordering::Relaxed);
        } else {
            metrics.publish_errors.fetch_add(1, Ordering::Relaxed);
        }
    }
    warn!(publisher_id, "publisher stopped because its queue closed");
}

fn spawn_metrics(
    worker_senders: Vec<Sender<OwnedNasdaqEvent>>,
    publisher_senders: Vec<Sender<PublishFrame>>,
    metrics: Arc<PipelineMetrics>,
    worker_counts: Arc<Vec<AtomicU64>>,
) {
    std::thread::Builder::new()
        .name("sharded-metrics".to_owned())
        .spawn(move || {
            let mut last_callbacks = 0;
            let mut last_callback_nanos = 0;
            let mut last_callback_latency_buckets = [0; CALLBACK_LATENCY_LABELS.len()];
            let mut last_published = 0;
            let mut last_worker_counts = vec![0; worker_counts.len()];
            loop {
                std::thread::sleep(std::time::Duration::from_secs(10));
                let callbacks = metrics.callbacks.load(Ordering::Relaxed);
                let callback_nanos = metrics.callback_nanos.load(Ordering::Relaxed);
                let published = metrics.published.load(Ordering::Relaxed);
                let delta_callbacks = callbacks.saturating_sub(last_callbacks);
                let delta_nanos = callback_nanos.saturating_sub(last_callback_nanos);
                let avg_callback_us = if delta_callbacks == 0 {
                    0.0
                } else {
                    delta_nanos as f64 / delta_callbacks as f64 / 1_000.0
                };
                let callback_latency_buckets: [u64; CALLBACK_LATENCY_LABELS.len()] =
                    std::array::from_fn(|bucket| {
                        let current =
                            metrics.callback_latency_buckets[bucket].load(Ordering::Relaxed);
                        let delta = current.saturating_sub(last_callback_latency_buckets[bucket]);
                        last_callback_latency_buckets[bucket] = current;
                        delta
                    });
                let worker_queue_sizes = worker_senders.iter().map(Sender::len).collect::<Vec<_>>();
                let publisher_queue_sizes = publisher_senders
                    .iter()
                    .map(Sender::len)
                    .collect::<Vec<_>>();
                let worker_events_per_sec = worker_counts
                    .iter()
                    .enumerate()
                    .map(|(worker, count)| {
                        let current = count.load(Ordering::Relaxed);
                        let rate = current.saturating_sub(last_worker_counts[worker]) as f64 / 10.0;
                        last_worker_counts[worker] = current;
                        rate
                    })
                    .collect::<Vec<_>>();
                info!(
                    callbacks_per_sec = delta_callbacks as f64 / 10.0,
                    avg_callback_us,
                    callback_latency_sample_every = CALLBACK_LATENCY_SAMPLE_EVERY,
                    callback_latency_labels = ?CALLBACK_LATENCY_LABELS,
                    ?callback_latency_buckets,
                    worker_handled = metrics.worker_handled.load(Ordering::Relaxed),
                    encoded = metrics.encoded.load(Ordering::Relaxed),
                    published_per_sec = published.saturating_sub(last_published) as f64 / 10.0,
                    total_published = published,
                    ignored = metrics.ignored.load(Ordering::Relaxed),
                    invalid_symbol = metrics.invalid_symbol.load(Ordering::Relaxed),
                    encode_errors = metrics.encode_errors.load(Ordering::Relaxed),
                    publish_errors = metrics.publish_errors.load(Ordering::Relaxed),
                    ?worker_queue_sizes,
                    ?worker_events_per_sec,
                    ?publisher_queue_sizes,
                    "sharded pipeline metrics"
                );
                last_callbacks = callbacks;
                last_callback_nanos = callback_nanos;
                last_published = published;
            }
        })
        .expect("spawn metrics thread");
}

#[cfg(test)]
mod tests {
    use super::*;
    use cfapi::value::CFValue;

    #[test]
    fn prefix_router_is_stable() {
        let router = PrefixRouter::new(16);
        let upper = SymbolKey::from_bytes(b"AAPL").unwrap();
        let lower = SymbolKey::from_bytes(b"aapl").unwrap();
        assert_eq!(router.route(upper), router.route(lower));
        assert_eq!(router.route(upper), 0);
        assert_eq!(router.route(SymbolKey::from_bytes(b"QCOM").unwrap()), 0);
    }

    #[test]
    fn symbol_key_rejects_oversize_symbols() {
        assert!(SymbolKey::from_bytes(&[b'A'; MAX_SYMBOL_BYTES + 1]).is_none());
    }

    #[test]
    fn snapshot_initializes_state_without_output() {
        let mut worker = WorkerState::new();
        let symbol = SymbolKey::from_bytes(b"AAPL").unwrap();
        let mut update = MessageUpdate::default();
        update.apply(10, CFValue::Double(101.0));
        update.apply(12, CFValue::Double(100.0));
        update.apply(20, CFValue::Int(1));
        let output = worker.process(OwnedNasdaqEvent {
            source: 533,
            event_type: SNAPSHOT_EVENT,
            symbol,
            update,
        });
        assert!(output.into_iter().all(|message| message.is_none()));
        assert_eq!(worker.states.len(), 1);
    }

    #[test]
    fn update_can_emit_tick_and_bidask() {
        let mut worker = WorkerState::new();
        let symbol = SymbolKey::from_bytes(b"AAPL").unwrap();
        let mut update = MessageUpdate::default();
        update.apply(8, CFValue::Double(100.5));
        update.apply(10, CFValue::Double(101.0));
        update.apply(12, CFValue::Double(100.0));
        update.apply(20, CFValue::Int(1));
        update.apply(1021, CFValue::Int(1));
        let output = worker.process(OwnedNasdaqEvent {
            source: 533,
            event_type: UPDATE_EVENT,
            symbol,
            update,
        });
        assert!(matches!(output[0], Some(NasdaqSolaceMessage::Tick(_))));
        assert!(matches!(output[1], Some(NasdaqSolaceMessage::BidAsk(_))));
    }

    #[test]
    fn same_symbol_from_two_sources_has_independent_state() {
        let mut worker = WorkerState::new();
        let symbol = SymbolKey::from_bytes(b"ETSY").unwrap();
        for source in [533, 534] {
            worker.process(OwnedNasdaqEvent {
                source,
                event_type: SNAPSHOT_EVENT,
                symbol,
                update: MessageUpdate::default(),
            });
        }
        assert_eq!(worker.states.len(), 2);
    }
}
