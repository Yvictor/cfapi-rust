use ahash::RandomState;
use cfapi::binding::MessageEvent;
use cfapi::event_reader::{EventReader, EventReaderSerConfig};
use cfapi::value::CFValue;
use dashmap::DashMap;

use crate::sink::Dest;
use super::Convertor;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use std::convert::Into;
use serde_repr::{Serialize_repr, Deserialize_repr};


#[derive(Debug, Clone, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum MarketPhase {
    PreMarket,
    Trading,
    PostMarket,
    Closed,
}

impl Default for MarketPhase {
    fn default() -> Self {
        MarketPhase::Closed
    }   
}

impl Into<MarketPhase> for i64 {
    fn into(self) -> MarketPhase {
        match self {
            1 => MarketPhase::Closed,
            2 => MarketPhase::PreMarket,
            4 => MarketPhase::Trading,
            10 => MarketPhase::PostMarket,
            _ => MarketPhase::Closed,
        }
    }
    
}



// snapshot
// '(207)ASK.CLOSE': 167.76,
// '(218)BID.CLOSE': 167.73,
// '(790)BID.CLOSE_SIZE': 2081,
// '(791)ASK.CLOSE_SIZE': 100,
// '(22)TRADE.VOL': 35490925,
// '(22)TRADE.VOL': 35490925,
// '(23)SESSION.VOL': 28884981,
// '(3945)BLOOMBERG.EXCH.CODE': 'UW',
// '(3947)BLOOMBERG.SEC.NUM.DES': 'AAPL',
// '(395)TRADE.OFFICIAL.LOW': 167.11,
// '(3950)SYMBOL.BLOOMBERG.TICKER': 'AAPL UW',
// '(400)TRADE.OPEN': 168.84,
//  '(401)TRADE.OFFICIAL.OPEN': 168.8,
//  '(8)TRADE.PRICE': 167.78,
//  '(826)ODD.LOT.IND': 0,
//  '(8506)INSTR.LOCAL_TYPE3': 'C',
//  '(854)TRADE.PART.CODE': 't',
//  '(9)TRADE.SIZE': 651,

//update


#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct DataNasdaqBasicState {
    exchange: String, // 3240
    // symbol: String, //
    code: String,      // 3170
    ts: f64,           // 16 utc time zone
    exchange_ts: i64,  // 55 exchange time zone
    ask_price: f64,    // 10
    ask_volume: i64,   // 11
    bid_price: f64,    // 12
    bid_volume: i64,   // 13
    close: f64,        // 447
    volume: i64,       // 448
    total_volume: i64, // 22 or use 463 for official vol
    total_amount: i64, // 460
    open: f64,
    high: f64,
    low: f64,
    market_phase: MarketPhase, // 1709 
    price_chg: f64, // 361
    pct_chg: f64,   // 362
                    // bid_side_total_vol: i64,
                    // ask_side_total_vol: i64,
                    // bid_side_total_cnt: i64,
                    // ask_side_total_cnt: i64,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct NBBidAsk {
    #[serde(skip_serializing)]
    _dest: String,
    exchange: String,
    code: String,
    ts: f64,
    ask_price: f64,  // f64[]
    ask_volume: i64, // i64[]
    bid_price: f64,  // f64[]
    bid_volume: i64, // i64
    market_phase: MarketPhase,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct NBTick {
    #[serde(skip_serializing)]
    _dest: String,
    exchange: String,
    code: String,
    ts: f64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    amount: i64,
    total_amount: i64,
    volume: i64,
    total_volume: i64,
    market_phase: MarketPhase,
    // tick_type:
    // chg_type:
    // price_change
    // pct_change
    // bid_side_total_vol
    // ask_side_total_vol
    // bid_side_total_cnt
    // ask_side_total_cnt
    // closing_oddlot_shares
    // fixed_trade_vol
    // suspend = 0,
    // simtrade = 0,
    // intraday_odd = 0
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum DataNasdaqBasicV1 {
    BidAsk(NBBidAsk),
    Tick(NBTick),
}

// Exchange.TSE
// Tick(
//     code = '2330',
//     datetime = datetime.datetime(2021, 7, 2, 13, 16, 55, 544646),
//     open = Decimal('591'),
//     avg_price = Decimal('590.24415'),
//     close = Decimal('590'),
//     high = Decimal('591'),
//     low = Decimal('589'),
//     amount = Decimal('276120'),
//     total_amount = Decimal('204995925'),
//     volume = 468,
//     total_volume = 347307,
//     tick_type = 1,
//     chg_type = 4,
//     price_chg = Decimal('-3'),
//     pct_chg = Decimal('-0.505902'),
//     bid_side_total_vol= 68209,
//     ask_side_total_vol = 279566,
//     bid_side_total_cnt = 28,
//     ask_side_total_cnt = 56,
//     closing_oddlot_shares = 0,
//     fixed_trade_vol = 0,
//     suspend = 0,
//     simtrade = 1,
//     intraday_odd = 1
// )

// Exchange.TSE
// BidAsk(
//     code = '2330',
//     datetime = datetime.datetime(2021, 7, 1, 9, 9, 54, 36828),
//     bid_price = [Decimal('593'), Decimal('592'), Decimal('591'), Decimal('590'), Decimal('589')],
//     bid_volume = [248, 180, 258, 267, 163],
//     diff_bid_vol = [3, 0, 0, 0, 0],
//     ask_price = [Decimal('594'), Decimal('595'), Decimal('596'), Decimal('597'), Decimal('598')],
//     ask_volume = [1457, 531, 506, 90, 259],
//     diff_ask_vol = [0, 0, 0, 0, 0],
//     suspend = 0,
//     simtrade = 0,
//     intraday_odd = 0
// )

pub struct NasdaqBasicConvertorV1 {
    reader_config: EventReaderSerConfig,
    state: DashMap<String, DataNasdaqBasicState, RandomState>,
}

impl NasdaqBasicConvertorV1 {
    pub fn new(reader_config: EventReaderSerConfig) -> Self {
        Self {
            reader_config,
            state: DashMap::with_hasher(RandomState::new()),
        }
    }
}

impl Default for NasdaqBasicConvertorV1 {
    fn default() -> Self {
        Self::new(
            EventReaderSerConfig::default()
                .with_event_type(true)
                .with_src(true),
        )
    }
}

impl Dest for DataNasdaqBasicV1 {
    fn get_dest(&self) -> &str {
        match self {
            DataNasdaqBasicV1::BidAsk(ba) => &ba._dest,
            DataNasdaqBasicV1::Tick(tick) => &tick._dest,
        }
    }
}


impl Convertor for NasdaqBasicConvertorV1 {
    type Out = DataNasdaqBasicV1;

    fn convert(&self, event: &MessageEvent) -> Option<Self::Out> {
        let src = i32::from(event.getSource());
        if src != 533 {
            let mut r = EventReader::new(event, &self.reader_config);
            let m = r.to_map();
            warn!("source not 533: {}, msg: {:?}", src, m);
            return None;
        }
        let symbol = event.getSymbol();
        let key = format!("{}.{}", src, symbol);
        // maybe consider event type for update or create
        // let event_type = event.getType();
        // debug!("key: {}, {}", key, event_type);
        // let status_code = i32::from(event.getStatusCode());
        // let tag = event.getTag();
        // debug!("event type: {:?}, status code: {}, tag: {}", event_type, (status_code), tag);
        let mut reader = EventReader::new(event, &self.reader_config);

        let data = match self.state.get_mut(&key) {
            Some(mut state) => {
                let mut is_tick = false;
                let mut is_bidask = false;
                for (token, value) in reader.iter_with_token_number() {
                    // TODO only update the field that has value
                    match token {
                        10 => {
                            is_bidask = true;
                            state.ask_price = value.to_f64()
                        },
                        11 => state.ask_volume = value.to_i64(),
                        12 => {
                            is_bidask = true;
                            state.bid_price = value.to_f64()
                        },
                        13 => state.bid_volume = value.to_i64(),
                        16 => state.ts = value.to_f64(),
                        55 => state.exchange_ts = value.to_i64(),
                        361 => state.price_chg = value.to_f64(),
                        362 => state.pct_chg = value.to_f64(),
                        447 => {
                            is_tick = true;
                            state.close = value.to_f64()
                        },
                        448 => state.volume = value.to_i64(),
                        // 460 => state.total_amount = value.to_i64(),
                        // 22 => state.amount = value.to_i64(),
                        463 => state.total_volume = value.to_i64(),
                        1709 => state.market_phase = value.to_i64().into(),
                        // 23 => update_int(value, &mut state.total_volume, "total_volume"),
                        _ => {
                            debug!("token: {}, value: {:?}", token, value);
                        }
                    }
                }
                // println!("updated state: {:?}", state.clone());
                let data = if is_tick {
                    Some(DataNasdaqBasicV1::Tick(NBTick {
                        _dest: format!("api/V1/TIC/{}/{}", state.exchange, state.code),
                        exchange: state.exchange.clone(),
                        code: state.code.clone(),
                        ts: state.ts,
                        open: state.open,
                        high: state.high,
                        low: state.low,
                        close: state.close,
                        amount: 0,
                        total_amount: state.total_amount,
                        volume: state.volume,
                        total_volume: state.total_volume,
                        market_phase: state.market_phase.clone(),
                    }))
                } else if is_bidask {
                    Some(DataNasdaqBasicV1::BidAsk(NBBidAsk {
                        _dest: format!("api/V1/QUO/{}/{}", state.exchange, state.code),
                        exchange: state.exchange.clone(),
                        code: state.code.clone(),
                        ts: state.ts,
                        ask_price: state.ask_price,
                        ask_volume: state.ask_volume,
                        bid_price: state.bid_price,
                        bid_volume: state.bid_volume,
                        market_phase: state.market_phase.clone(),
                    }))
                } else {
                    None
                };
                // println!("new data: {:?}", data);
                data
                // Some(data)
            }
            None => {
                // let mut r = EventReader::new(event, &self.reader_config);
                // let m = r.to_map();
                // println!("event map: {:?}", m);
                let data = DataNasdaqBasicState {
                    code: symbol.to_string(),
                    ask_price: reader.find(10).unwrap_or(CFValue::Double(0.0)).to_f64(),
                    ask_volume: reader.find(11).unwrap_or(CFValue::Double(0.0)).to_i64(),
                    bid_price: reader.find(12).unwrap_or(CFValue::Double(0.0)).to_f64(),
                    bid_volume: reader.find(13).unwrap_or(CFValue::Double(0.0)).to_i64(),
                    ts: reader.find(16).unwrap_or(CFValue::Datetime(0.0)).to_f64(),
                    // total_volume: reader.find(22).unwrap_or(CFValue::Int(0)).to_i64(),
                    exchange_ts: reader.find(55).unwrap_or(CFValue::Int(0)).to_i64(),
                    price_chg: reader.find(361).unwrap_or(CFValue::Double(0.0)).to_f64(),
                    pct_chg: reader.find(362).unwrap_or(CFValue::Double(0.0)).to_f64(),
                    high: reader.find(389).unwrap_or(CFValue::Double(0.0)).to_f64(), // 389 is official high 388 is ice not exise in pre market
                    low: reader.find(395).unwrap_or(CFValue::Double(0.0)).to_f64(), // 395 is official low 394 is ice not exise in pre market
                    open: reader.find(401).unwrap_or(CFValue::Double(0.0)).to_f64(),// 401 is official open not exise in pre market
                    close: reader.find(447).unwrap_or(CFValue::Double(0.0)).to_f64(),
                    volume: reader.find(448).unwrap_or(CFValue::Int(0)).to_i64(),
                    total_amount: reader.find(460).unwrap_or(CFValue::Int(0)).to_i64(),
                    total_volume: reader.find(463).unwrap_or(CFValue::Int(0)).to_i64(),
                    market_phase: reader.find(1709).unwrap_or(CFValue::Int(1)).to_i64().into(),
                    exchange: reader.find(3240).unwrap_or(CFValue::String("".into())).to_string(),
                };
                // println!("new data: {:?}", data);
                self.state.insert(key.clone(), data);
                None
            }
        };
        // None
        data
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cfapi::value::CFValue;
    use std::collections::BTreeMap;

    #[test]
    fn test_dashmap_nasdaq_basic() {
        let state = DashMap::with_hasher(RandomState::new());
        let data = DataNasdaqBasicState {
            exchange: "TSE".into(),
            code: "2330".into(),
            ts: 1625203015.544646,
            exchange_ts: 1625203015,
            ask_price: 594.0,
            ask_volume: 1457,
            bid_price: 593.0,
            bid_volume: 248,
            open: 591.0,
            high: 591.0,
            low: 589.0,
            close: 590.0,
            volume: 468,
            total_volume: 347307,
            total_amount: 204995925,
            price_chg: -3.0,
            pct_chg: -0.505902,
            market_phase: MarketPhase::Closed,
        };
        state.insert("TSE.2330", data.clone());
        let origin = state.get("TSE.2330").unwrap();
        assert_eq!(origin.ask_price, 594.0);
        let _new_v = match state.get_mut("TSE.2330") {
            Some(mut v) => {
                v.ask_price = 595.0;
            }
            None => {
                assert!(false);
            }
        };
        let new_v = state.get("TSE.2330").unwrap();
        assert_eq!(new_v.ask_price, 595.0);
    }

    #[test]
    fn test_dashmap_usage() {
        let state = DashMap::with_hasher(RandomState::new());
        let mut map = BTreeMap::new();
        map.insert("abc", CFValue::Double(0.5));
        map.insert("vvv", CFValue::Int(1));
        state.insert("src.symbol", map);
        let mut new_map = BTreeMap::new();
        new_map.insert("new", CFValue::String("value".into()));
        // let vaule = state.get_mut("symbol.src");
        let new_v = match state.get_mut("src.symbol") {
            Some(mut v) => {
                v.extend(new_map);
                v.clone()
            }
            None => {
                println!("src.symbol not found");
                new_map
            }
        };
        print!("state: {:?}", state);
        println!("{:?}", new_v);
        let new_v = state.get("src.symbol");
        match new_v {
            Some(v) => {
                let new_field = v.get("new");
                match new_field {
                    Some(f) => match *f {
                        CFValue::String(ref s) => {
                            assert_eq!(s, "value");
                        }
                        _ => {
                            assert!("cf value not string." == "");
                        }
                    },
                    None => {
                        assert!("new field not exist." == "");
                    }
                }
            }
            None => {
                assert!(false);
            }
        }
    }
}
