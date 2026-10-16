//! Physics: the rigid-body backend, as a Plugin.
//!
//! Rigid bodies, colliders and the transform write-back run on Rapier.
//!
//! `tests/physics.rs` is the gate: gravity over N steps, a box coming to
//! rest, two identical runs agreeing bit for bit, a static body that does not move, and
//! an entity without a rigid body that `step` does not touch.

mod physics_system3d;
mod plugin;
mod shape;

pub use physics_system3d::{BodyHandle, Physics3D, GRAVITY};
pub use plugin::FemurPlugin;
pub use shape::ColliderShape;
