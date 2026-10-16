//! Events: the bus, the key and mouse codes, the event enum.

mod bus;
mod event;
mod key;

pub use bus::{EventBus, Subscription};
pub use event::Event;
pub use key::{KeyCode, MouseButton};
