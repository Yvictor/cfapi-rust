use ahash::RandomState;
use cfapi::binding::MessageEvent;
use cfapi::event_reader::{EventReader, EventReaderSerConfig};
use cfapi::value::CFValue;
use dashmap::DashMap;

use super::Convertor;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use std::convert::Into;


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MarketPhase {
    Closed = 1,
    PreMarket = 2,
    Trading = 4,
    PostMarket = 10,
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
pub struct DataNasdaqBasic {
    exchange: String, // 3240
    // symbol: String, //
    code: String,      // 3170
    ts: f64,           // 16 utc time zone
    exchange_ts: i64,  // 55 exchange time zone
    ask_price: f64,    // 10
    ask_volume: i64,   // 11
    bid_price: f64,    // 12
    bid_volume: i64,   // 13
    price: f64,        // 447
    volume: i64,       // 448
    total_volume: i64, // 22 or use 463 for official vol
    total_amount: i64, // 460
    // open: f64,
    // high: f64,
    // low: f64,
    market_phase: MarketPhase, // 1709 
    price_chg: f64, // 361
    pct_chg: f64,   // 362
                    // bid_side_total_vol: i64,
                    // ask_side_total_vol: i64,
                    // bid_side_total_cnt: i64,
                    // ask_side_total_cnt: i64,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct DataNasdaqBasicBidAsk {
    exchange: String,
    code: String,
    ts: f64,
    ask_price: f64,  // f64[]
    ask_volume: i64, // i64[]
    bid_price: f64,  // f64[]
    bid_volume: i64, // i64
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct DataNasdaqBasicTrade {
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

pub struct NasdaqBasicConvertor {
    reader_config: EventReaderSerConfig,
    state: DashMap<String, DataNasdaqBasic, RandomState>,
}

impl NasdaqBasicConvertor {
    pub fn new(reader_config: EventReaderSerConfig) -> Self {
        Self {
            reader_config,
            state: DashMap::with_hasher(RandomState::new()),
        }
    }
}

impl Default for NasdaqBasicConvertor {
    fn default() -> Self {
        Self::new(
            EventReaderSerConfig::default()
                .with_event_type(true)
                .with_src(true),
        )
    }
}

fn update_int(value: CFValue, v: &mut i64, field_name: &str) {
    match value {
        CFValue::Int(vv) => {
            *v = vv;
        }
        _ => {
            warn!("{} not int: {:?} ignore update", field_name, value);
        }
    }
}

fn update_double(value: CFValue, v: &mut f64, field_name: &str) {
    match value {
        CFValue::Double(vv) => {
            *v = vv;
        }
        _ => {
            warn!("{} not double: {:?} ignore update", field_name, value);
        }
    }
}

impl Convertor for NasdaqBasicConvertor {
    type Out = Option<DataNasdaqBasic>;

    fn convert(&self, event: &MessageEvent) -> Self::Out {
        let src = i32::from(event.getSource());
        let symbol = event.getSymbol();
        let key = format!("{}.{}", src, symbol);
        // maybe consider event type for update or create
        // let event_type = event.getType();
        // debug!("key: {}, {}", key, event_type);
        // let status_code = i32::from(event.getStatusCode());
        // let tag = event.getTag();
        // debug!("event type: {:?}, status code: {}, tag: {}", event_type, (status_code), tag);
        let mut reader = EventReader::new(event, &self.reader_config);

        let updated_map = match self.state.get_mut(&key) {
            Some(mut state) => {
                for (token, value) in reader.iter_with_token_number() {
                    // TODO only update the field that has value
                    match token {
                        10 => state.ask_price = value.to_f64(),
                        11 => state.ask_volume = value.to_i64(),
                        12 => state.bid_price = value.to_f64(),
                        13 => state.bid_volume = value.to_i64(),
                        16 => state.ts = value.to_f64(),
                        55 => state.exchange_ts = value.to_i64(),
                        361 => state.price_chg = value.to_f64(),
                        362 => state.pct_chg = value.to_f64(),
                        447 => state.price = value.to_f64(),
                        448 => state.total_volume = value.to_i64(),
                        1709 => state.market_phase = value.to_i64().into(),
                        // 23 => update_int(value, &mut state.total_volume, "total_volume"),
                        _ => {
                            debug!("token: {}, value: {:?}", token, value);
                        }
                    }
                }
                Some(state.clone())
            }
            None => {
                let mut r = EventReader::new(event, &self.reader_config);
                let m = r.to_map();
                println!("event map: {:?}", m);
                let data = DataNasdaqBasic {
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
                    price: reader.find(447).unwrap_or(CFValue::Double(0.0)).to_f64(),
                    volume: reader.find(448).unwrap_or(CFValue::Int(0)).to_i64(),
                    total_amount: reader.find(460).unwrap_or(CFValue::Int(0)).to_i64(),
                    total_volume: reader.find(463).unwrap_or(CFValue::Int(0)).to_i64(),
                    market_phase: reader.find(1709).unwrap_or(CFValue::Int(1)).to_i64().into(),
                    exchange: reader.find(3240).unwrap_or(CFValue::String("".into())).to_string(),
                };
                println!("new data: {:?}", data);
                self.state.insert(key.clone(), data);
                None
            }
        };
        updated_map
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
        let data = DataNasdaqBasic {
            exchange: "TSE".into(),
            code: "2330".into(),
            ts: 1625203015.544646,
            exchange_ts: 1625203015,
            ask_price: 594.0,
            ask_volume: 1457,
            bid_price: 593.0,
            bid_volume: 248,
            price: 590.0,
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
