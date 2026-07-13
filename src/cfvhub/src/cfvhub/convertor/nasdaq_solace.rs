use ahash::RandomState;
use cfapi::binding::MessageEvent;
use cfapi::event_reader::{EventReader, EventReaderSerConfig};
use cfapi::value::CFValue;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};

use super::Convertor;
use crate::sink::Dest;

const NASDAQ_SOURCES: &[i32] = &[533, 534];

#[derive(Debug, Default, Clone)]
pub(crate) struct SymbolState {
    code: String,
    datetime: Option<String>,
    exchange_datetime: Option<String>,
    open: Option<f64>,
    avg_price: Option<f64>,
    close: Option<f64>,
    high: Option<f64>,
    low: Option<f64>,
    total_amount: Option<f64>,
    volume: Option<u64>,
    total_volume: Option<u64>,
    bid_price: Option<f64>,
    bid_volume: Option<u64>,
    ask_price: Option<f64>,
    ask_volume: Option<u64>,
    chg_type: Option<u8>,
    price_chg: Option<f64>,
    pct_chg: Option<f64>,
    bid_side_total_vol: u64,
    ask_side_total_vol: u64,
    bid_side_total_cnt: u64,
    ask_side_total_cnt: u64,
    market_phase: Option<u8>,
    tradable_status: Option<u8>,
    trade_cond: Option<u64>,
    pub(crate) serial_num: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NasdaqTick {
    #[serde(skip_serializing)]
    dest: String,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NasdaqBidAsk {
    #[serde(skip_serializing)]
    dest: String,
    code: String,
    datetime: String,
    exchange_datetime: String,
    bid_price: Vec<String>,
    bid_volume: Vec<u64>,
    ask_price: Vec<String>,
    ask_volume: Vec<u64>,
    market_phase: u8,
    tradable_status: u8,
    #[serde(rename = "SerialNum")]
    serial_num: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum NasdaqSolaceMessage {
    Tick(NasdaqTick),
    BidAsk(NasdaqBidAsk),
}

impl Dest for NasdaqSolaceMessage {
    fn get_dest(&self) -> &str {
        match self {
            NasdaqSolaceMessage::Tick(tick) => &tick.dest,
            NasdaqSolaceMessage::BidAsk(bidask) => &bidask.dest,
        }
    }
}

pub struct NasdaqSolaceConvertorV1 {
    reader_config: EventReaderSerConfig,
    state: DashMap<String, SymbolState, RandomState>,
}

impl Default for NasdaqSolaceConvertorV1 {
    fn default() -> Self {
        Self::new(EventReaderSerConfig::default())
    }
}

impl NasdaqSolaceConvertorV1 {
    pub fn new(reader_config: EventReaderSerConfig) -> Self {
        Self {
            reader_config,
            state: DashMap::with_hasher(RandomState::new()),
        }
    }
}

impl Convertor for NasdaqSolaceConvertorV1 {
    type Out = NasdaqSolaceMessage;

    fn convert(&self, event: &MessageEvent) -> Option<Self::Out> {
        let src = i32::from(event.getSource());
        if !NASDAQ_SOURCES.contains(&src) {
            return None;
        }

        let symbol = event.getSymbol().to_string();
        let state_key = format!("{}.{}", src, symbol);
        let mut reader = EventReader::new(event, &self.reader_config);
        let mut update = MessageUpdate::default();
        for (token, value) in reader.iter_with_token_number() {
            update.apply(token, value);
        }

        // TODO: Confirm whether fields that arrive without token 1021 or 20 should be ignored
        // completely, or whether any source sends required carry-forward state only there.
        if !update.is_tick && !update.is_bidask {
            return None;
        }

        let mut state = self.state.entry(state_key).or_insert_with(|| SymbolState {
            code: symbol.clone(),
            ..SymbolState::default()
        });
        state.apply(&update);

        if update.is_tick {
            state.serial_num = state.serial_num.saturating_add(1);
            return Some(NasdaqSolaceMessage::Tick(state.to_tick()));
        }
        if update.is_bidask {
            state.serial_num = state.serial_num.saturating_add(1);
            return Some(NasdaqSolaceMessage::BidAsk(state.to_bidask()));
        }
        None
    }
}

#[derive(Debug, Default)]
pub(crate) struct MessageUpdate {
    is_tick: bool,
    is_bidask: bool,
    datetime: Option<String>,
    exchange_datetime: Option<String>,
    open: Option<f64>,
    avg_price: Option<f64>,
    close: Option<f64>,
    high: Option<f64>,
    low: Option<f64>,
    total_amount: Option<f64>,
    volume: Option<u64>,
    total_volume: Option<u64>,
    bid_price: Option<f64>,
    bid_volume: Option<u64>,
    ask_price: Option<f64>,
    ask_volume: Option<u64>,
    chg_type: Option<u8>,
    price_chg: Option<f64>,
    pct_chg: Option<f64>,
    market_phase: Option<u8>,
    tradable_status: Option<u8>,
    trade_cond: Option<u64>,
}

impl MessageUpdate {
    pub(crate) fn apply(&mut self, token: i32, value: CFValue) {
        match token {
            5 => {}
            8 | 447 => self.close = value_f64(value),
            9 | 448 => self.volume = value_u64(value),
            10 => self.ask_price = value_f64(value),
            11 => self.ask_volume = value_u64(value),
            12 => self.bid_price = value_f64(value),
            13 => self.bid_volume = value_u64(value),
            20 => self.is_bidask = true,
            16 => self.datetime = value_string(value),
            55 => self.exchange_datetime = value_string(value),
            316 => self.chg_type = value_u8(value),
            361 => self.price_chg = value_f64(value),
            362 => self.pct_chg = value_f64(value),
            388 => self.high = value_f64(value),
            394 => self.low = value_f64(value),
            400 => self.open = value_f64(value),
            460 => self.total_amount = value_f64(value),
            463 => self.total_volume = value_u64(value),
            474 => self.avg_price = value_f64(value),
            1021 => self.is_tick = true,
            1708 => self.tradable_status = value_u8(value),
            1709 => self.market_phase = value_u8(value),
            2500 => self.trade_cond = value_u64(value),
            _ => {}
        }
    }
}

impl SymbolState {
    pub(crate) fn new(code: String) -> Self {
        Self {
            code,
            ..Self::default()
        }
    }

    pub(crate) fn apply(&mut self, update: &MessageUpdate) {
        update_field(&mut self.datetime, update.datetime.clone());
        update_field(
            &mut self.exchange_datetime,
            update.exchange_datetime.clone(),
        );
        update_field(&mut self.open, update.open);
        update_field(&mut self.avg_price, update.avg_price);
        update_field(&mut self.close, update.close);
        update_field(&mut self.high, update.high);
        update_field(&mut self.low, update.low);
        update_field(&mut self.total_amount, update.total_amount);
        update_field(&mut self.volume, update.volume);
        update_field(&mut self.total_volume, update.total_volume);
        update_field(&mut self.bid_price, update.bid_price);
        update_field(&mut self.bid_volume, update.bid_volume);
        update_field(&mut self.ask_price, update.ask_price);
        update_field(&mut self.ask_volume, update.ask_volume);
        update_field(&mut self.chg_type, update.chg_type);
        update_field(&mut self.price_chg, update.price_chg);
        update_field(&mut self.pct_chg, update.pct_chg);
        update_field(&mut self.market_phase, update.market_phase);
        update_field(&mut self.tradable_status, update.tradable_status);
        update_field(&mut self.trade_cond, update.trade_cond);

        if update.is_tick {
            let tick_type = self.tick_type();
            let volume = update.volume.or(self.volume).unwrap_or(0);
            match tick_type {
                1 => {
                    self.bid_side_total_vol = self.bid_side_total_vol.saturating_add(volume);
                    self.bid_side_total_cnt = self.bid_side_total_cnt.saturating_add(1);
                }
                2 => {
                    self.ask_side_total_vol = self.ask_side_total_vol.saturating_add(volume);
                    self.ask_side_total_cnt = self.ask_side_total_cnt.saturating_add(1);
                }
                _ => {}
            }
        }
    }

    pub(crate) fn to_tick(&self) -> NasdaqTick {
        let close = self.close.unwrap_or_default();
        let volume = self.volume.unwrap_or_default();
        NasdaqTick {
            dest: format!("IS/v2/TIC/NASDAQ/{}", self.code),
            code: self.code.clone(),
            datetime: self.datetime.clone().unwrap_or_default(),
            exchange_datetime: self.exchange_datetime.clone().unwrap_or_default(),
            open: decimal(self.open),
            avg_price: decimal(self.avg_price),
            close: decimal(self.close),
            high: decimal(self.high),
            low: decimal(self.low),
            amount: amount(volume, close),
            total_amount: decimal(self.total_amount),
            volume,
            total_volume: self.total_volume.unwrap_or_default(),
            tick_type: self.tick_type(),
            chg_type: self
                .chg_type
                .unwrap_or_else(|| chg_type(self.open, self.close)),
            price_chg: decimal(self.price_chg.or_else(|| price_chg(self.open, self.close))),
            pct_chg: decimal(self.pct_chg.or_else(|| pct_chg(self.open, self.close))),
            bid_side_total_vol: self.bid_side_total_vol,
            ask_side_total_vol: self.ask_side_total_vol,
            bid_side_total_cnt: self.bid_side_total_cnt,
            ask_side_total_cnt: self.ask_side_total_cnt,
            market_phase: self.market_phase.unwrap_or_default(),
            tradable_status: self.tradable_status.unwrap_or_default(),
            trade_cond: self.trade_cond.unwrap_or_default(),
            serial_num: self.serial_num,
        }
    }

    pub(crate) fn to_bidask(&self) -> NasdaqBidAsk {
        NasdaqBidAsk {
            dest: format!("IS/v2/QUO/NASDAQ/{}", self.code),
            code: self.code.clone(),
            datetime: self.datetime.clone().unwrap_or_default(),
            exchange_datetime: self.exchange_datetime.clone().unwrap_or_default(),
            bid_price: decimal_vec(self.bid_price),
            bid_volume: uint_vec(self.bid_volume),
            ask_price: decimal_vec(self.ask_price),
            ask_volume: uint_vec(self.ask_volume),
            market_phase: self.market_phase.unwrap_or_default(),
            tradable_status: self.tradable_status.unwrap_or_default(),
            serial_num: self.serial_num,
        }
    }

    fn tick_type(&self) -> u8 {
        let Some(close) = self.close else {
            return 0;
        };
        if self.ask_price.is_some_and(|ask| close >= ask) {
            1
        } else if self.bid_price.is_some_and(|bid| close <= bid) {
            2
        } else {
            0
        }
    }
}

impl MessageUpdate {
    pub(crate) fn is_tick(&self) -> bool {
        self.is_tick
    }

    pub(crate) fn is_bidask(&self) -> bool {
        self.is_bidask
    }
}

fn update_field<T>(target: &mut Option<T>, value: Option<T>) {
    if value.is_some() {
        *target = value;
    }
}

fn value_f64(value: CFValue) -> Option<f64> {
    match value {
        CFValue::Double(value) | CFValue::Datetime(value) => Some(value),
        CFValue::Int(value) => Some(value as f64),
        CFValue::String(value) => value.parse().ok(),
        CFValue::Unknown => None,
    }
}

fn value_u64(value: CFValue) -> Option<u64> {
    match value {
        CFValue::Int(value) => value.try_into().ok(),
        CFValue::Double(value) | CFValue::Datetime(value) => nonnegative_f64_to_u64(value),
        CFValue::String(value) => value.parse().ok(),
        CFValue::Unknown => None,
    }
}

fn value_u8(value: CFValue) -> Option<u8> {
    value_u64(value).and_then(|value| value.try_into().ok())
}

fn value_string(value: CFValue) -> Option<String> {
    match value {
        CFValue::String(value) => Some(value),
        CFValue::Double(value) | CFValue::Datetime(value) => Some(decimal(Some(value))),
        CFValue::Int(value) => Some(value.to_string()),
        CFValue::Unknown => None,
    }
}

fn decimal(value: Option<f64>) -> String {
    let Some(value) = value else {
        return String::new();
    };
    if !value.is_finite() {
        return String::new();
    }
    let text = format!("{value:.10}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn decimal_vec(value: Option<f64>) -> Vec<String> {
    value
        .map(|value| vec![decimal(Some(value))])
        .unwrap_or_default()
}

fn uint_vec(value: Option<u64>) -> Vec<u64> {
    value.map(|value| vec![value]).unwrap_or_default()
}

fn amount(volume: u64, close: f64) -> u64 {
    if !close.is_finite() || close <= 0.0 {
        return 0;
    }
    (volume as f64 * close).round() as u64
}

fn nonnegative_f64_to_u64(value: f64) -> Option<u64> {
    if value.is_finite() && value >= 0.0 {
        Some(value.round() as u64)
    } else {
        None
    }
}

fn price_chg(open: Option<f64>, close: Option<f64>) -> Option<f64> {
    Some(close? - open?)
}

fn pct_chg(open: Option<f64>, close: Option<f64>) -> Option<f64> {
    let open = open?;
    if open == 0.0 {
        return None;
    }
    Some((close? - open) / open * 100.0)
}

fn chg_type(open: Option<f64>, close: Option<f64>) -> u8 {
    let Some(chg) = price_chg(open, close) else {
        return 0;
    };
    if chg > 0.0 {
        1
    } else if chg < 0.0 {
        2
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_trims_trailing_zeroes() {
        assert_eq!(decimal(Some(12.340000)), "12.34");
        assert_eq!(decimal(Some(12.0)), "12");
        assert_eq!(decimal(None), "");
    }

    #[test]
    fn computes_tick_type_from_quote() {
        let state = SymbolState {
            close: Some(10.0),
            ask_price: Some(10.0),
            bid_price: Some(9.9),
            ..SymbolState::default()
        };
        assert_eq!(state.tick_type(), 1);

        let state = SymbolState {
            close: Some(9.9),
            ask_price: Some(10.0),
            bid_price: Some(9.9),
            ..SymbolState::default()
        };
        assert_eq!(state.tick_type(), 2);
    }

    #[test]
    fn uses_requested_destinations() {
        let state = SymbolState {
            code: "AAPL".to_string(),
            ..SymbolState::default()
        };
        assert_eq!(state.to_tick().dest, "IS/v2/TIC/NASDAQ/AAPL");
        assert_eq!(state.to_bidask().dest, "IS/v2/QUO/NASDAQ/AAPL");
    }
}
