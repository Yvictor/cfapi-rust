use serde::Serialize;
use std::time::{Duration, Instant};

#[derive(Debug, Serialize, Clone)]
struct TickPayload {
    code: String,
    datetime: String,
    exchange_datetime: String,
    open: String,
    avg_price: String,
    close: String,
    high: String,
    low: String,
    amount: u64,
    total_amount: String,
    volume: u64,
    total_volume: u64,
    tick_type: u8,
    chg_type: u8,
    price_chg: String,
    pct_chg: String,
    bid_side_total_vol: u64,
    ask_side_total_vol: u64,
    bid_side_total_cnt: u64,
    ask_side_total_cnt: u64,
    market_phase: u8,
    tradable_status: u8,
    trade_cond: u64,
    #[serde(rename = "SerialNum")]
    serial_num: u64,
}

#[derive(Debug, Serialize, Clone)]
struct BidAskPayload {
    code: String,
    datetime: String,
    exchange_datetime: String,
    bid_price: Vec<String>,
    bid_volume: Vec<u64>,
    ask_price: Vec<String>,
    ask_volume: Vec<u64>,
    market_phase: u8,
    tradable_status: u8,
}

#[derive(Debug, Serialize, Clone)]
#[serde(untagged)]
enum MessagePayload {
    Tick(TickPayload),
    BidAsk(BidAskPayload),
}

#[derive(Clone)]
struct SymbolState {
    code: String,
    datetime: String,
    exchange_datetime: String,
    open: f64,
    avg_price: f64,
    close: f64,
    high: f64,
    low: f64,
    total_amount: f64,
    volume: u64,
    total_volume: u64,
    bid_price: f64,
    bid_volume: u64,
    ask_price: f64,
    ask_volume: u64,
    chg_type: u8,
    price_chg: f64,
    pct_chg: f64,
    bid_side_total_vol: u64,
    ask_side_total_vol: u64,
    bid_side_total_cnt: u64,
    ask_side_total_cnt: u64,
    market_phase: u8,
    tradable_status: u8,
    trade_cond: u64,
    serial_num: u64,
}

impl SymbolState {
    fn apply_tick(&mut self, i: u64) -> MessagePayload {
        self.close = 190.0 + (i % 100) as f64 * 0.01;
        self.volume = 1 + (i % 1000);
        self.total_volume = self.total_volume.wrapping_add(self.volume);
        self.total_amount += self.close * self.volume as f64;
        self.price_chg = self.close - self.open;
        self.pct_chg = if self.open == 0.0 {
            0.0
        } else {
            self.price_chg / self.open * 100.0
        };
        self.chg_type = if self.price_chg > 0.0 {
            1
        } else if self.price_chg < 0.0 {
            2
        } else {
            0
        };
        let tick_type = self.tick_type();
        match tick_type {
            1 => {
                self.bid_side_total_vol = self.bid_side_total_vol.wrapping_add(self.volume);
                self.bid_side_total_cnt = self.bid_side_total_cnt.wrapping_add(1);
            }
            2 => {
                self.ask_side_total_vol = self.ask_side_total_vol.wrapping_add(self.volume);
                self.ask_side_total_cnt = self.ask_side_total_cnt.wrapping_add(1);
            }
            _ => {}
        }
        self.serial_num = self.serial_num.wrapping_add(1);
        MessagePayload::Tick(self.to_tick())
    }

    fn apply_bidask(&mut self, i: u64) -> MessagePayload {
        self.bid_price = 190.0 + (i % 100) as f64 * 0.01;
        self.ask_price = self.bid_price + 0.01;
        self.bid_volume = 100 + (i % 10_000);
        self.ask_volume = 120 + (i % 10_000);
        MessagePayload::BidAsk(self.to_bidask())
    }

    fn to_tick(&self) -> TickPayload {
        TickPayload {
            code: self.code.clone(),
            datetime: self.datetime.clone(),
            exchange_datetime: self.exchange_datetime.clone(),
            open: decimal(self.open),
            avg_price: decimal(self.avg_price),
            close: decimal(self.close),
            high: decimal(self.high),
            low: decimal(self.low),
            amount: (self.volume as f64 * self.close).round() as u64,
            total_amount: decimal(self.total_amount),
            volume: self.volume,
            total_volume: self.total_volume,
            tick_type: self.tick_type(),
            chg_type: self.chg_type,
            price_chg: decimal(self.price_chg),
            pct_chg: decimal(self.pct_chg),
            bid_side_total_vol: self.bid_side_total_vol,
            ask_side_total_vol: self.ask_side_total_vol,
            bid_side_total_cnt: self.bid_side_total_cnt,
            ask_side_total_cnt: self.ask_side_total_cnt,
            market_phase: self.market_phase,
            tradable_status: self.tradable_status,
            trade_cond: self.trade_cond,
            serial_num: self.serial_num,
        }
    }

    fn to_bidask(&self) -> BidAskPayload {
        BidAskPayload {
            code: self.code.clone(),
            datetime: self.datetime.clone(),
            exchange_datetime: self.exchange_datetime.clone(),
            bid_price: vec![decimal(self.bid_price)],
            bid_volume: vec![self.bid_volume],
            ask_price: vec![decimal(self.ask_price)],
            ask_volume: vec![self.ask_volume],
            market_phase: self.market_phase,
            tradable_status: self.tradable_status,
        }
    }

    fn tick_type(&self) -> u8 {
        if self.close >= self.ask_price {
            1
        } else if self.close <= self.bid_price {
            2
        } else {
            0
        }
    }
}

fn main() {
    let iterations = std::env::var("CFVHUB_BENCH_ITERS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(5_000_000);
    let symbols = std::env::var("CFVHUB_BENCH_SYMBOLS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(5_000);

    bench(
        "tick_state_msgpack",
        iterations,
        symbols,
        Workload::TickMsgpack,
    );
    bench(
        "bidask_state_msgpack",
        iterations,
        symbols,
        Workload::BidAskMsgpack,
    );
    bench(
        "mixed_state_msgpack",
        iterations,
        symbols,
        Workload::MixedMsgpack,
    );
    bench(
        "mixed_state_only",
        iterations,
        symbols,
        Workload::MixedStateOnly,
    );
}

#[derive(Clone, Copy)]
enum Workload {
    TickMsgpack,
    BidAskMsgpack,
    MixedMsgpack,
    MixedStateOnly,
}

fn bench(name: &str, iterations: u64, symbol_count: usize, workload: Workload) {
    let mut states = (0..symbol_count)
        .map(|i| sample_state(format!("A{:04}", i)))
        .collect::<Vec<_>>();
    let started = Instant::now();
    let mut bytes = 0usize;
    for i in 0..iterations {
        let state = &mut states[i as usize % symbol_count];
        match workload {
            Workload::TickMsgpack => {
                let payload = state.apply_tick(i);
                bytes = bytes.wrapping_add(rmp_serde::to_vec(&payload).unwrap().len());
            }
            Workload::BidAskMsgpack => {
                let payload = state.apply_bidask(i);
                bytes = bytes.wrapping_add(rmp_serde::to_vec(&payload).unwrap().len());
            }
            Workload::MixedMsgpack => {
                let payload = if i % 2 == 0 {
                    state.apply_tick(i)
                } else {
                    state.apply_bidask(i)
                };
                bytes = bytes.wrapping_add(rmp_serde::to_vec(&payload).unwrap().len());
            }
            Workload::MixedStateOnly => {
                if i % 2 == 0 {
                    let _ = state.apply_tick(i);
                } else {
                    let _ = state.apply_bidask(i);
                }
            }
        }
    }
    report(name, iterations, symbol_count, started.elapsed(), bytes);
}

fn report(name: &str, iterations: u64, symbol_count: usize, elapsed: Duration, bytes: usize) {
    let secs = elapsed.as_secs_f64();
    println!(
        "bench={} iterations={} symbols={} elapsed_sec={:.6} msg_per_sec={:.2} avg_us={:.3} bytes={}",
        name,
        iterations,
        symbol_count,
        secs,
        iterations as f64 / secs,
        secs * 1_000_000.0 / iterations as f64,
        bytes
    );
}

fn sample_state(code: String) -> SymbolState {
    SymbolState {
        code,
        datetime: "1782120480.1708600521".to_string(),
        exchange_datetime: "52800163".to_string(),
        open: 190.12,
        avg_price: 190.34,
        close: 190.56,
        high: 191.0,
        low: 189.5,
        total_amount: 123456789.12,
        volume: 100,
        total_volume: 9_876_543,
        bid_price: 190.55,
        bid_volume: 1200,
        ask_price: 190.56,
        ask_volume: 1300,
        chg_type: 1,
        price_chg: 0.44,
        pct_chg: 0.2314,
        bid_side_total_vol: 456_789,
        ask_side_total_vol: 567_890,
        bid_side_total_cnt: 1234,
        ask_side_total_cnt: 2345,
        market_phase: 4,
        tradable_status: 1,
        trade_cond: 27,
        serial_num: 42,
    }
}

fn decimal(value: f64) -> String {
    if !value.is_finite() {
        return String::new();
    }
    let text = format!("{value:.10}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}
