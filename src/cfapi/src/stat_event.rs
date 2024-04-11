use super::binding::{StatisticsEvent, StatisticsEvent_StatsTypes};
use tracing::info;
use serde::{Serialize, Deserialize};
pub trait StatisticsEventHandlerExt {
    fn on_statistics_event(&mut self, event: &StatisticsEvent);
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatisticsData {
    msgs_in: u64,
    msgs_out: u64,
    drop: u64,
    csp_drop: u64,
    pct_full: u64,
    peak_pct_full: u64,
    in_msgs_sec_100ms: u64,
    peak_in_msgs_sec_100ms: u64,
    in_msgs_sec: u64,
    peak_in_msgs_sec: u64,
    out_msgs_sec_100ms: u64,
    peak_out_msgs_sec_100ms: u64,
    out_msgs_sec: u64,
    peak_out_msgs_sec: u64,
    net_msgs_out: u64,
    net_out_msgs_sec_100ms: u64,
    peak_net_out_msgs_sec_100ms: u64,
    net_out_msgs_sec: u64,
    peak_net_out_msgs_sec: u64,
}

impl std::convert::From<&StatisticsEvent> for StatisticsData {
    fn from(value: &StatisticsEvent) -> Self {
        StatisticsData{
            msgs_in: value.getStat(StatisticsEvent_StatsTypes::MSGS_IN),
            msgs_out: value.getStat(StatisticsEvent_StatsTypes::MSGS_OUT),
            drop: value.getStat(StatisticsEvent_StatsTypes::DROP),
            csp_drop: value.getStat(StatisticsEvent_StatsTypes::CSP_DROP),
            pct_full: value.getStat(StatisticsEvent_StatsTypes::PCT_FULL),
            peak_pct_full: value.getStat(StatisticsEvent_StatsTypes::PEAK_PCT_FULL),
            in_msgs_sec_100ms: value.getStat(StatisticsEvent_StatsTypes::IN_MSGS_SEC_100MS),
            peak_in_msgs_sec_100ms: value.getStat(StatisticsEvent_StatsTypes::PEAK_IN_MSGS_SEC_100MS),
            in_msgs_sec: value.getStat(StatisticsEvent_StatsTypes::IN_MSGS_SEC),
            peak_in_msgs_sec: value.getStat(StatisticsEvent_StatsTypes::PEAK_IN_MSGS_SEC),
            out_msgs_sec_100ms: value.getStat(StatisticsEvent_StatsTypes::OUT_MSGS_SEC_100MS),
            peak_out_msgs_sec_100ms: value.getStat(StatisticsEvent_StatsTypes::PEAK_OUT_MSGS_SEC_100MS),
            out_msgs_sec: value.getStat(StatisticsEvent_StatsTypes::OUT_MSGS_SEC),
            peak_out_msgs_sec: value.getStat(StatisticsEvent_StatsTypes::PEAK_OUT_MSGS_SEC),
            net_msgs_out: value.getStat(StatisticsEvent_StatsTypes::NET_MSGS_OUT),
            net_out_msgs_sec_100ms: value.getStat(StatisticsEvent_StatsTypes::NET_OUT_MSGS_SEC_100MS),
            peak_net_out_msgs_sec_100ms: value.getStat(StatisticsEvent_StatsTypes::PEAK_NET_OUT_MSGS_SEC_100MS),
            net_out_msgs_sec: value.getStat(StatisticsEvent_StatsTypes::NET_OUT_MSGS_SEC),
            peak_net_out_msgs_sec: value.getStat(StatisticsEvent_StatsTypes::PEAK_NET_OUT_MSGS_SEC),

        }
    }
}

pub struct DefaultStatisticsEventHandler;

impl StatisticsEventHandlerExt for DefaultStatisticsEventHandler {
    fn on_statistics_event(&mut self, event: &StatisticsEvent) {
        let data: StatisticsData = event.into();
        info!("statistics event: {:?}", data);
    }
}

impl Default for Box<dyn StatisticsEventHandlerExt> {
    fn default() -> Self {
        Box::new(DefaultStatisticsEventHandler)
    }
}
