//! Job system and task graph.
//!
//! Two memory-ordering invariants are load-bearing: the decrement release
//! sequence in `JobCounter::dec` and the final decrement in `resolve`. Both are
//! documented at their definitions.

mod jobs;
mod taskgraph;

pub use jobs::{Job, JobCounter, Jobs};
pub use taskgraph::{TaskGraph, TaskId};
