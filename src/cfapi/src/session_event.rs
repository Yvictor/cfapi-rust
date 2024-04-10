use super::binding::{SessionEvent, SessionEvent_Types};
use tracing::info;

pub trait SessionEventHandlerExt {
    fn on_session_event(&mut self, event: &SessionEvent);
}

pub struct DefaultSessionEventHandler;

impl SessionEventHandlerExt for DefaultSessionEventHandler {
    fn on_session_event(&mut self, event: &SessionEvent) {
        match event.getType() {
            SessionEvent_Types::CFAPI_SESSION_UNAVAILABLE => {
                info!("session unavailable");
            }
            SessionEvent_Types::CFAPI_SESSION_ESTABLISHED => {
                info!("session established");
            }
            SessionEvent_Types::CFAPI_SESSION_RECOVERY => {
                info!("session recovery");
            }
            SessionEvent_Types::CFAPI_SESSION_RECOVERY_SOURCES => {
                info!("session recovery sourceID = {:?}", event.getSourceID());
            }
            SessionEvent_Types::CFAPI_CDD_LOADED => {
                info!("cdd loaded version: {}", event.getCddVersion());
            }
            SessionEvent_Types::CFAPI_SESSION_AVAILABLE_ALLSOURCES => {
                let source_id = event.getSourceID();
                info!("session available all sources sourceID = {:?}", source_id);
            }
            SessionEvent_Types::CFAPI_SESSION_AVAILABLE_SOURCES => {
                let source_id = event.getSourceID();
                info!("session available sources sourceID = {:?}", source_id);
            }
            SessionEvent_Types::CFAPI_SESSION_RECEIVE_QUEUE_ABOVE_THRESHOLD => {
                info!(
                    "session receive queue above threshold {:?}",
                    event.getQueueDepth()
                );
            }
            SessionEvent_Types::CFAPI_SESSION_RECEIVE_QUEUE_BELOW_THRESHOLD => {
                info!(
                    "session receive queue below threshold {:?}",
                    event.getQueueDepth()
                );
            }
            SessionEvent_Types::CFAPI_SESSION_JIT_START_CONFLATING => {
                info!("session jit start conflation");
            }
            SessionEvent_Types::CFAPI_SESSION_JIT_STOP_CONFLATING => {
                info!("session jit stop conflation");
            }
            SessionEvent_Types::CFAPI_SESSION_SOURCE_ADDED => {
                info!("session source added sourceID = {:?}", event.getSourceID());
            }
            SessionEvent_Types::CFAPI_SESSION_SOURCE_REMOVED => {
                info!(
                    "session source removed sourceID = {:?}",
                    event.getSourceID()
                );
            }
        }
    }
}

impl Default for Box<dyn SessionEventHandlerExt> {
    fn default() -> Self {
        Box::new(DefaultSessionEventHandler)
    }
}
