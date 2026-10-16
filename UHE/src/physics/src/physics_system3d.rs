//! The 3D world: bodies in, transforms out.
//!
//! Sync is per step, both directions, in that order: scene -> Rapier, step, Rapier ->
//! scene. Two markers make an unchanged body cost nothing and a user edit survive:
//!
//! * `Entry::source`: everything the body was built from. Equal to the scene's current
//!   description means the body is left alone; a difference rebuilds it.
//! * `Entry::written`: the pose this backend last put in the scene. A scene pose that
//!   disagrees is an editor edit, and it is pushed into Rapier rather than overwritten by
//!   the next step.

use std::collections::HashMap;

use adam::PhysicsBackend;
use glam::Vec3;
use meats::{Collider, Entity, RigidBody, RigidBodyKind, Scene, Transform};
use rapier3d::prelude::{
    BroadPhaseBvh, CCDSolver, ColliderBuilder, ColliderSet, ImpulseJointSet, IntegrationParameters,
    IslandManager, MultibodyJointSet, NarrowPhase, PhysicsPipeline, Pose, RigidBodyBuilder,
    RigidBodyHandle, RigidBodySet, SoftBodySet,
};

use crate::shape::{Refusal, ShapeSlot};

/// Jolt's default gravity, which the 3D world used. Box2D's `-9.8` is the 2D value,
/// and this backend is 3D only.
pub const GRAVITY: Vec3 = Vec3::new(0.0, -9.81, 0.0);

/// Defaults for a body whose collider component does not override them.
const DEFAULT_FRICTION: f32 = 0.2;
const DEFAULT_RESTITUTION: f32 = 0.0;

/// A body in the Rapier world.
///
/// The id stays in the backend and the scene component stays a description, so a
/// serialised scene never carries a stale index, and `meats` stays free of any
/// dependency on `rapier3d`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BodyHandle(RigidBodyHandle);

/// Rigid-body simulation on Rapier.
pub struct Physics3D {
    pipeline: PhysicsPipeline,
    params: IntegrationParameters,
    islands: IslandManager,
    broad_phase: BroadPhaseBvh,
    narrow_phase: NarrowPhase,
    bodies: RigidBodySet,
    colliders: ColliderSet,
    impulse_joints: ImpulseJointSet,
    multibody_joints: MultibodyJointSet,
    soft_bodies: SoftBodySet,
    ccd_solver: CCDSolver,
    entries: HashMap<Entity, Entry>,
    generation: u64,
}

impl Physics3D {
    pub fn new() -> Self {
        Self {
            pipeline: PhysicsPipeline::new(),
            // One fixed parameter set for the world's whole life: the step length is the
            // only thing `step` touches. That, and one pipeline, is what makes two runs of
            // the same scene produce the same numbers: Rapier holds no generator of its
            // own, so there is nothing else to seed.
            params: IntegrationParameters::default(),
            islands: IslandManager::new(),
            broad_phase: BroadPhaseBvh::new(),
            narrow_phase: NarrowPhase::new(),
            bodies: RigidBodySet::new(),
            colliders: ColliderSet::new(),
            impulse_joints: ImpulseJointSet::new(),
            multibody_joints: MultibodyJointSet::new(),
            soft_bodies: SoftBodySet::new(),
            ccd_solver: CCDSolver::new(),
            entries: HashMap::new(),
            generation: 0,
        }
    }

    /// The body an entity currently has, if any.
    ///
    /// The editor wants this to show a body's state and a game wants it to apply an
    /// impulse; both go through a handle rather than through a scene component, so a scene
    /// file never records a body that a reload cannot reproduce.
    pub fn handle(&self, entity: Entity) -> Option<BodyHandle> {
        Some(BodyHandle(self.entries.get(&entity)?.body?))
    }

    fn sync_in(&mut self, scene: &Scene) {
        self.generation += 1;
        let generation = self.generation;

        for entity in scene.entities() {
            let Some((source, pose)) = read_source(scene, entity) else {
                continue;
            };

            // Two markers, one decision: an equal source means keep the body, a different
            // pose means the scene moved it (an editor edit, or a script), and neither
            // means there is nothing to do at all. The push is deferred because it needs
            // `self.bodies` while `entry` borrows `self.entries`.
            let mut rebuild = false;
            let mut push = None;
            if let Some(entry) = self.entries.get_mut(&entity) {
                if entry.source == source {
                    entry.seen = generation;
                    if entry.written != pose {
                        entry.written = pose;
                        push = entry.body.map(|handle| (handle, pose));
                    }
                } else {
                    rebuild = true;
                }
            } else {
                rebuild = true;
            }

            if rebuild {
                self.rebuild(entity, source, pose, generation);
            } else if let Some((handle, pose)) = push {
                self.push_pose(handle, pose);
            }
        }

        let dead: Vec<Entity> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.seen != generation)
            .map(|(entity, _)| *entity)
            .collect();
        for entity in dead {
            self.despawn(entity);
        }
    }

    fn sync_out(&mut self, scene: &mut Scene) {
        for entity in scene.entities() {
            let Some(entry) = self.entries.get_mut(&entity) else {
                continue;
            };
            // Static bodies are never moved by the simulation, and skipping them leaves
            // a static body's pose bit-identical to what the scene asked for.
            if entry.source.kind == RigidBodyKind::Static {
                continue;
            }
            let Some(handle) = entry.body else {
                continue;
            };
            let Some(body) = self.bodies.get(handle) else {
                continue;
            };
            let pose = scene_pose(body.position());
            entry.written = pose;
            write_pose(scene, entity, pose);
        }
    }

    fn rebuild(&mut self, entity: Entity, source: Source, pose: ScenePose, generation: u64) {
        self.despawn(entity);

        let shape = match source.shape.shared() {
            Ok(shape) => shape,
            Err(Refusal::Unsupported(name)) => {
                log::warn!("femur: entity {entity:?}: collider shape {name} is unsupported");
                self.track_refused(entity, source, pose, generation);
                return;
            }
            Err(Refusal::Missing) => {
                log::debug!("femur: entity {entity:?}: rigid body with no collider; no body");
                self.track_refused(entity, source, pose, generation);
                return;
            }
        };

        let body = match source.kind {
            RigidBodyKind::Static => RigidBodyBuilder::fixed(),
            RigidBodyKind::Kinematic => RigidBodyBuilder::kinematic_position_based(),
            RigidBodyKind::Dynamic => RigidBodyBuilder::dynamic(),
        }
        .pose(rapier_pose(pose))
        .linvel(source.velocity)
        .gravity_scale(source.gravity_scale)
        // One `damping` on the component drives both linear and angular damping.
        .linear_damping(source.damping)
        .angular_damping(source.damping);
        let handle = self.bodies.insert(body);

        let mut collider = ColliderBuilder::new(shape)
            .friction(source.friction)
            .restitution(source.restitution)
            .translation(source.offset);
        if source.kind == RigidBodyKind::Dynamic {
            // Jolt set the body's mass from the component and left inertia to the
            // shape; a Rapier collider mass is the same split.
            collider = collider.mass(source.mass);
        }
        self.colliders
            .insert_with_parent(collider, handle, &mut self.bodies);

        self.entries.insert(
            entity,
            Entry {
                body: Some(handle),
                source,
                written: pose,
                seen: generation,
            },
        );
    }

    /// Tracks an entity whose collider was refused, so the refusal is reported when the
    /// description changes rather than once per frame.
    fn track_refused(&mut self, entity: Entity, source: Source, pose: ScenePose, seen: u64) {
        self.entries.insert(
            entity,
            Entry {
                body: None,
                source,
                written: pose,
                seen,
            },
        );
    }

    fn push_pose(&mut self, handle: RigidBodyHandle, pose: ScenePose) {
        let Some(body) = self.bodies.get_mut(handle) else {
            return;
        };
        body.set_position(rapier_pose(pose), true);
    }

    fn despawn(&mut self, entity: Entity) {
        let Some(entry) = self.entries.remove(&entity) else {
            return;
        };
        let Some(handle) = entry.body else {
            return;
        };
        // `true`: an entity that lost its `RigidBody` should not leave its collider in
        // the world.
        self.bodies.remove(
            handle,
            &mut self.islands,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            &mut self.soft_bodies,
            true,
        );
    }
}

impl Default for Physics3D {
    fn default() -> Self {
        Self::new()
    }
}

impl PhysicsBackend for Physics3D {
    fn name(&self) -> &'static str {
        "rapier3d"
    }

    fn step(&mut self, scene: &mut Scene, dt: f32) {
        self.sync_in(scene);

        // The engine's accumulator decides the rate; this only takes what it is given,
        // clamped at zero so a bad delta cannot run time backwards (Rapier treats a zero
        // step as "apply the changes, detect collisions, advance nothing").
        self.params.dt = dt.max(0.0);

        // Rapier's solver already generates no constraint between two fixed bodies, so
        // no collision layers are needed to exclude that pair.
        self.pipeline.step(
            GRAVITY,
            &self.params,
            &mut self.islands,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.bodies,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            &mut self.soft_bodies,
            &mut self.ccd_solver,
            &(),
            &(),
        );

        self.sync_out(scene);
    }

    fn body_count(&self) -> usize {
        self.bodies.len()
    }
}

struct Entry {
    /// `None` when the entity's collider was refused; the entity stays tracked so the
    /// refusal is reported once, when the description appears or changes.
    body: Option<RigidBodyHandle>,
    /// What the body was built from.
    source: Source,
    /// The pose this backend last wrote into the scene.
    written: ScenePose,
    /// The sync pass that last saw the entity's rigid body.
    seen: u64,
}

/// Everything about an entity that decides the body it gets. A change rebuilds it.
#[derive(PartialEq)]
struct Source {
    kind: RigidBodyKind,
    mass: f32,
    velocity: Vec3,
    damping: f32,
    gravity_scale: f32,
    friction: f32,
    restitution: f32,
    offset: Vec3,
    shape: ShapeSlot,
}

/// A pose as the scene stores one: a translation, and `Transform::rotation`, Euler
/// **degrees** in the `Rz*Ry*Rx` order `Transform::rotation_quat` is the forward half
/// of.
///
/// The comparison that catches an editor edit is between this and what the backend last
/// wrote, so it stays in the scene's own representation: writing the angles back is what
/// makes the two sides bit-equal for an untouched body, where comparing a quaternion
/// against a re-derived one would leave every resting body looking edited and awake it
/// forever.
#[derive(Clone, Copy, PartialEq)]
struct ScenePose {
    translation: Vec3,
    rotation: Vec3,
}

/// Euler degrees to Rapier's quaternion, through `meats`' own conversion.
///
/// `Transform::rotation_quat` and not `Quat::from_euler(EulerRot::XYZ)`: the scene stores
/// the `Rz*Ry*Rx` order, the composition glam names `EulerRot::XYZEx`, not the `Rx*Ry*Rz`
/// of its `XYZ`. Either way the convention is `meats`', so the conversion lives there and
/// this only takes it.
fn rapier_pose(pose: ScenePose) -> Pose {
    let transform = Transform {
        translation: Vec3::ZERO,
        rotation: pose.rotation,
        scale: Vec3::ONE,
    };
    Pose::from_parts(pose.translation, transform.rotation_quat())
}

/// Rapier's quaternion back to Euler degrees, through `meats`' own conversion.
///
/// `meats::euler_degrees` is the inverse of `Transform::rotation_quat`, so the two
/// directions are one convention kept in the crate that owns it; glam's `to_euler` is the
/// inverse of a different order and would hand the scene a different orientation.
fn scene_pose(pose: &Pose) -> ScenePose {
    ScenePose {
        translation: pose.translation,
        rotation: meats::euler_degrees(pose.rotation),
    }
}

/// The physics view of an entity: its body description, and where it currently is.
fn read_source(scene: &Scene, entity: Entity) -> Option<(Source, ScenePose)> {
    let built = scene.get(entity)?;
    let rigid_body = built.get::<RigidBody>()?;
    let transform = built.get::<Transform>()?;

    let pose = ScenePose {
        translation: transform.translation,
        rotation: transform.rotation,
    };

    let collider = built.get::<Collider>();
    let source = Source {
        kind: rigid_body.kind,
        mass: rigid_body.mass,
        velocity: rigid_body.velocity,
        damping: rigid_body.damping,
        gravity_scale: rigid_body.gravity_scale,
        friction: collider.as_ref().map_or(DEFAULT_FRICTION, |c| c.friction),
        restitution: collider
            .as_ref()
            .map_or(DEFAULT_RESTITUTION, |c| c.restitution),
        offset: collider.as_ref().map_or(Vec3::ZERO, |c| c.offset),
        // The component's `Offset` becomes the collider's position on the body, which is
        // what the field is for.
        shape: ShapeSlot::from_collider(collider.as_deref(), transform.scale),
    };
    Some((source, pose))
}

/// Writes the simulated pose back, leaving the scale alone.
///
/// The scene stores Euler angles, so both directions go through the one convention
/// `meats` defines.
fn write_pose(scene: &mut Scene, entity: Entity, pose: ScenePose) {
    let Some(mut built) = scene.get_mut(entity) else {
        return;
    };
    let Some(transform) = built.get_mut::<Transform>() else {
        return;
    };
    transform.translation = pose.translation;
    transform.rotation = pose.rotation;
}
