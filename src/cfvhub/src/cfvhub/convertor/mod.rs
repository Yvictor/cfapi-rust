use cfapi::binding::MessageEvent;
use serde::Serialize;
// Stateless
// Stateful
pub trait Convertor {
    type Out: Serialize;

    fn convert(&self, event: &MessageEvent) -> Self::Out;
}

pub mod stateless_map;
pub mod stateful_map;
pub mod nasdaq_basic;

// pub trait Convertor<Out> {
//     fn convert(&self, event: &MessageEvent) -> Out;
// }
