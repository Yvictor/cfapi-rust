use ahash::RandomState;
use cfapi::binding::MessageEvent;
use cfapi::event_reader::{EventReader, EventReaderSerConfig};
use cfapi::value::CFValue;
use dashmap::DashMap;

use super::Convertor;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

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
    exchange: String,
    symbol: String,
    ts: f64,
    ask_price: f64,
    ask_volume: i64,
    bid_price: f64,
    bid_volume: i64,
    price: f64,
    volume: i64,
    total_volume: i64,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct DataNasdaqBasicBidAsk {
    exchange: String,
    symbol: String,
    ts: f64,
    ask_price: f64,
    ask_volume: i64,
    bid_price: f64,
    bid_volume: i64,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct DataNasdaqBasicTrade {
    exchange: String,
    symbol: String,
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
    type Out = DataNasdaqBasic;

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
                    match token {
                        207 => update_double(value, &mut state.ask_price, "ask_price"),
                        218 => update_double(value, &mut state.bid_price, "bid_price"),
                        791 => update_int(value, &mut state.ask_volume, "ask_volume"),
                        790 => update_int(value, &mut state.bid_volume, "bid_volume"),
                        22 => update_int(value, &mut state.volume, "volume"),
                        23 => update_int(value, &mut state.total_volume, "total_volume"),
                        _ => {
                            debug!("token: {}, value: {:?}", token, value);
                        }
                    }
                }

                state.clone()
            }
            None => {
                let data = DataNasdaqBasic {
                    exchange: "".into(), //reader.find(3945).unwrap_or("".to_string()),
                    symbol: symbol.to_string(),
                    ts: 0.0,
                    ask_price: 0.0,
                    ask_volume: 0,
                    bid_price: 0.0,
                    bid_volume: 0,
                    price: 0.0,
                    volume: 0,
                    total_volume: 0,
                };
                data
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
