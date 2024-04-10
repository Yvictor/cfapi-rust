use super::binding::{UserEvent, UserEvent_Types};
use tracing::info;

pub trait UserEventHandlerExt {
    fn on_user_event(&mut self, event: &UserEvent);
}



pub struct DefaultUserEventHandler;

impl UserEventHandlerExt for DefaultUserEventHandler {
    fn on_user_event(&mut self, event: &UserEvent) {
        match event.getType() {
            UserEvent_Types::AUTHORIZATION_FAILURE => {
                info!("AUTHORIZATION_FAILURE");
            }
            UserEvent_Types::AUTHORIZATION_SUCCESS => {
                info!("AUTHORIZATION_SUCCESS");
            }
        }
    }
}

impl Default for Box<dyn UserEventHandlerExt> {
    fn default() -> Self {
        Box::new(DefaultUserEventHandler)
    }
}
