//! Base engine. Owns the composition root and nothing else.
//!
//! The structural rule: plugins hold behavior, `Engine` holds data. A plugin
//! owns its state and gets `&mut Engine` per hook, so `adam` depends on no
//! plugin crate and no plugin reaches into another's state.

pub mod app;
pub mod backend;
pub mod engine;
pub mod error;
pub mod logging;
pub mod plugin;
pub mod time;

pub use app::App;
pub use backend::{AudioBackend, Listener, PhysicsBackend, VoiceId};
pub use engine::Engine;
pub use error::{Error, Result};
pub use plugin::{FrameCtx, Plugin};
pub use time::Time;

pub use chassis;
pub use grind;
