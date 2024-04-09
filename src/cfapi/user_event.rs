use crate::cfapi::binding::UserEvent;

pub trait UserEventHandlerExt {
    fn on_user_event(&mut self, event: &UserEvent);
}
