use cfapi::binding::MessageEvent;
use serde::Serialize;
// Stateless
// Stateful
pub trait Convertor {
    type Out: Serialize;

    fn convert(&self, event: &MessageEvent) -> Option<Self::Out>;
}

pub mod nasdaq_basic;
pub mod stateful_map;
pub mod stateless_map;

// pub trait Convertor<Out> {
//     fn convert(&self, event: &MessageEvent) -> Out;
// }
