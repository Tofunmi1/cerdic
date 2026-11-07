//! The physics world: bodies, the fixed step, poses, velocities and contacts.
use rapier3d::na::{Isometry3, Quaternion, Translation3, UnitQuaternion, Vector3};
use rapier3d::prelude::{
    CCDSolver, CoefficientCombineRule, ColliderHandle, ColliderSet, DefaultBroadPhase, Group,
    ImpulseJointHandle, ImpulseJointSet, IntegrationParameters, InteractionGroups, IslandManager,
    MultibodyJointSet, NarrowPhase, PhysicsPipeline, QueryPipeline, RigidBodyBuilder,
    RigidBodyHandle, RigidBodySet, RigidBodyType,
};

use crate::{BodyDesc, BodyId, Error, Kind, MAX_BODIES, STEP};

mod query;
pub use query::{RayHit, ShapeHit, SurfaceHit};

/// Where a body is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    /// Position of the body's centre in metres.
    pub position: [f32; 3],
    /// Orientation as a unit quaternion `[x, y, z, w]`.
    pub rotation: [f32; 4],
}

struct Entry {
    handle: RigidBodyHandle,
    collider: ColliderHandle,
    /// False once the body was removed: its slot stays so every other body keeps its id.
    live: bool,
}

/// A set of rigid bodies stepped at 1/60 s.
pub struct World {
    pub(crate) gravity: [f32; 3],
    parameters: IntegrationParameters,
    pipeline: PhysicsPipeline,
    islands: IslandManager,
    broad_phase: DefaultBroadPhase,
    narrow_phase: NarrowPhase,
    pub(crate) bodies: RigidBodySet,
    pub(crate) colliders: ColliderSet,
    /// Spatial queries for the walker, refreshed when it moves.
    pub(crate) query: QueryPipeline,
    query_dirty: bool,
    /// The walking character, if the scene has one.
    pub(crate) walker: Option<crate::walker::Walker>,
    pub(crate) impulse_joints: ImpulseJointSet,
    multibody_joints: MultibodyJointSet,
    ccd: CCDSolver,
    entries: Vec<Entry>,
    /// Joints added so far, in order.
    pub(crate) joints: Vec<ImpulseJointHandle>,
    /// Springs, applied as forces each step.
    pub(crate) springs: Vec<crate::joint::Spring>,
    /// A fixed body at the origin that joints to "the world" attach to, made when first needed.
    anchor: Option<RigidBodyHandle>,
}

fn isometry(position: [f32; 3], rotation: [f32; 4]) -> Isometry3<f32> {
    let [x, y, z, w] = rotation;
    Isometry3::from_parts(
        Translation3::new(position[0], position[1], position[2]),
        UnitQuaternion::from_quaternion(Quaternion::new(w, x, y, z)),
    )
}

fn body_type(kind: Kind) -> RigidBodyType {
    match kind {
        Kind::Static => RigidBodyType::Fixed,
        Kind::Dynamic => RigidBodyType::Dynamic,
        Kind::Kinematic => RigidBodyType::KinematicPositionBased,
    }
}

impl World {
    /// An empty world with this gravity (metres per second squared; Earth is `[0, -9.81, 0]`).
    #[must_use]
    pub fn new(gravity: [f32; 3]) -> Self {
        Self {
            gravity,
            // Eight solver iterations instead of four keep chains of joints (a bridge, a pendulum with a weight) from stretching.
            parameters: IntegrationParameters {
                dt: STEP,
                num_solver_iterations: std::num::NonZeroUsize::MIN.saturating_add(7),
                ..IntegrationParameters::default()
            },
            pipeline: PhysicsPipeline::new(),
            islands: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            bodies: RigidBodySet::new(),
            colliders: ColliderSet::new(),
            query: QueryPipeline::new(),
            query_dirty: true,
            walker: None,
            impulse_joints: ImpulseJointSet::new(),
            multibody_joints: MultibodyJointSet::new(),
            ccd: CCDSolver::new(),
            entries: Vec::new(),
            joints: Vec::new(),
            springs: Vec::new(),
            anchor: None,
        }
    }

    /// Adds a body.
    ///
    /// # Errors
    /// [`Error::TooManyBodies`], [`Error::BadShape`] for a shape that cannot be built or a mesh on a moving body, and
    /// [`Error::BadValue`] for a number that is not finite or out of range.
    pub fn add(&mut self, desc: &BodyDesc) -> Result<BodyId, Error> {
        if self.len() >= MAX_BODIES {
            return Err(Error::TooManyBodies);
        }
        let numbers = [
            desc.density,
            desc.friction,
            desc.restitution,
            desc.linear_damping,
            desc.angular_damping,
        ];
        let finite = numbers
            .iter()
            .chain(&desc.position)
            .chain(&desc.rotation)
            .all(|v| v.is_finite());
        if !finite
            || desc.density <= 0.0
            || desc.friction < 0.0
            || !(0.0..=1.0).contains(&desc.restitution)
            || desc.linear_damping < 0.0
            || desc.angular_damping < 0.0
        {
            return Err(Error::BadValue(
                "density above 0, friction and damping from 0, restitution 0 to 1",
            ));
        }
        if desc.kind != Kind::Static && !desc.shape.can_move() {
            return Err(Error::BadShape(
                "a mesh or heightfield can only be static; use a convex shape for moving bodies",
            ));
        }
        // A contact bounces as much as the bouncier of the two, so a rubber ball bounces on a floor that says nothing
        // about bounce (the default would average it with the floor's zero).
        let collider = desc
            .shape
            .collider()?
            .density(desc.density)
            .friction(desc.friction)
            .restitution(desc.restitution)
            .restitution_combine_rule(CoefficientCombineRule::Max)
            .collision_groups(InteractionGroups::new(
                Group::from_bits_retain(desc.layers.member),
                Group::from_bits_retain(desc.layers.mask),
            ))
            .sensor(desc.sensor);
        let builder = match desc.kind {
            Kind::Static => RigidBodyBuilder::fixed(),
            Kind::Dynamic => RigidBodyBuilder::dynamic(),
            Kind::Kinematic => RigidBodyBuilder::kinematic_position_based(),
        };
        let body = builder
            .position(isometry(desc.position, desc.rotation))
            .linear_damping(desc.linear_damping)
            .angular_damping(desc.angular_damping)
            .build();
        let handle = self.bodies.insert(body);
        let collider = self
            .colliders
            .insert_with_parent(collider, handle, &mut self.bodies);
        self.entries.push(Entry {
            handle,
            collider,
            live: true,
        });
        self.query_dirty = true;
        Ok(BodyId(self.entries.len() - 1))
    }

    /// Removes a body with its collider and every joint and spring attached to it (a character that left the game takes its ropes
    /// and hair chains with it). The ids of the other bodies do not change; this id is not reused and answers [`Error::NoSuchBody`].
    ///
    /// # Errors
    /// [`Error::NoSuchBody`].
    pub fn remove(&mut self, id: BodyId) -> Result<(), Error> {
        let handle = self.entry(id)?.handle;
        self.springs
            .retain(|spring| spring.first != handle && spring.second != handle);
        self.bodies.remove(
            handle,
            &mut self.islands,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            true,
        );
        // Rapier dropped the joints attached to the body; forget their handles.
        let joints = &self.impulse_joints;
        self.joints.retain(|joint| joints.get(*joint).is_some());
        self.entries[id.0].live = false;
        self.query_dirty = true;
        Ok(())
    }

    pub(crate) fn entry_handle(&self, id: BodyId) -> Result<RigidBodyHandle, Error> {
        self.entry(id).map(|entry| entry.handle)
    }

    /// The fixed body that joints to the world hang from.
    pub(crate) fn world_anchor(&mut self) -> RigidBodyHandle {
        if let Some(handle) = self.anchor {
            return handle;
        }
        let handle = self.bodies.insert(RigidBodyBuilder::fixed().build());
        self.anchor = Some(handle);
        handle
    }

    fn entry(&self, id: BodyId) -> Result<&Entry, Error> {
        self.entries
            .get(id.0)
            .filter(|entry| entry.live)
            .ok_or(Error::NoSuchBody)
    }

    /// How many bodies the world holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.iter().filter(|entry| entry.live).count()
    }

    /// Whether the world has no bodies.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Advances the world by one fixed step.
    pub fn step(&mut self) {
        self.apply_springs();
        self.pipeline.step(
            &Vector3::new(self.gravity[0], self.gravity[1], self.gravity[2]),
            &self.parameters,
            &mut self.islands,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.bodies,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            &mut self.ccd,
            None,
            &(),
            &(),
        );
        self.query_dirty = true;
    }

    /// Where a body is now.
    ///
    /// # Errors
    /// [`Error::NoSuchBody`].
    pub fn pose(&self, id: BodyId) -> Result<Pose, Error> {
        let body = &self.bodies[self.entry(id)?.handle];
        let (t, q) = (body.translation(), body.rotation().coords);
        Ok(Pose {
            position: [t.x, t.y, t.z],
            rotation: [q.x, q.y, q.z, q.w],
        })
    }

    /// Moves a kinematic body to this pose by the end of the next step (so it pushes what it touches); any other body
    /// is placed there at once with its speeds cleared.
    ///
    /// # Errors
    /// [`Error::NoSuchBody`], or [`Error::BadValue`] for a pose that is not finite.
    pub fn set_pose(
        &mut self,
        id: BodyId,
        position: [f32; 3],
        rotation: [f32; 4],
    ) -> Result<(), Error> {
        if !position.iter().chain(&rotation).all(|v| v.is_finite()) {
            return Err(Error::BadValue("a pose must be finite"));
        }
        let handle = self.entry(id)?.handle;
        let target = isometry(position, rotation);
        let body = &mut self.bodies[handle];
        if body.body_type() == RigidBodyType::KinematicPositionBased {
            body.set_next_kinematic_position(target);
        } else {
            body.set_position(target, true);
            body.set_linvel(Vector3::zeros(), true);
            body.set_angvel(Vector3::zeros(), true);
        }
        self.query_dirty = true;
        Ok(())
    }

    /// Changes how a body moves (a held object becomes kinematic, then dynamic again when let go).
    ///
    /// # Errors
    /// [`Error::NoSuchBody`], or [`Error::BadShape`] when a triangle-mesh body would start to move.
    pub fn set_kind(&mut self, id: BodyId, kind: Kind) -> Result<(), Error> {
        let collider = self.entry(id)?.collider;
        let is_mesh = self.colliders.get(collider).is_some_and(|c| {
            c.shape().as_trimesh().is_some() || c.shape().as_heightfield().is_some()
        });
        if is_mesh && kind != Kind::Static {
            return Err(Error::BadShape("a mesh or heightfield can only be static"));
        }
        let handle = self.entry(id)?.handle;
        self.bodies[handle].set_body_type(body_type(kind), true);
        Ok(())
    }

    /// Sets a body's linear and angular velocity (a throw).
    ///
    /// # Errors
    /// [`Error::NoSuchBody`], or [`Error::BadValue`] for a speed that is not finite.
    pub fn set_velocity(
        &mut self,
        id: BodyId,
        linear: [f32; 3],
        angular: [f32; 3],
    ) -> Result<(), Error> {
        if !linear.iter().chain(&angular).all(|v| v.is_finite()) {
            return Err(Error::BadValue("a velocity must be finite"));
        }
        let handle = self.entry(id)?.handle;
        let body = &mut self.bodies[handle];
        body.set_linvel(Vector3::new(linear[0], linear[1], linear[2]), true);
        body.set_angvel(Vector3::new(angular[0], angular[1], angular[2]), true);
        Ok(())
    }

    /// Changes a body's linear velocity by `change` metres per second, waking it, and leaves its spin alone (a gust of wind, a buoyant
    /// lift, a magnet: whatever pushes from outside).
    ///
    /// # Errors
    /// [`Error::NoSuchBody`], or [`Error::BadValue`] for a change that is not finite.
    pub fn push(&mut self, id: BodyId, change: [f32; 3]) -> Result<(), Error> {
        if !change.iter().all(|v| v.is_finite()) {
            return Err(Error::BadValue("a push must be finite"));
        }
        let handle = self.entry(id)?.handle;
        let body = &mut self.bodies[handle];
        let now = *body.linvel();
        body.set_linvel(now + Vector3::new(change[0], change[1], change[2]), true);
        Ok(())
    }

    /// A body's linear velocity in metres per second.
    ///
    /// # Errors
    /// [`Error::NoSuchBody`].
    pub fn velocity(&self, id: BodyId) -> Result<[f32; 3], Error> {
        let body = &self.bodies[self.entry(id)?.handle];
        // A sleeping body is at rest, whatever velocity it had when it fell asleep.
        if body.is_sleeping() {
            return Ok([0.0; 3]);
        }
        let v = body.linvel();
        Ok([v.x, v.y, v.z])
    }

    /// The pairs of bodies touching after the last step, each as `(lower id, higher id)`, in id order.
    #[must_use]
    pub fn contacts(&self) -> Vec<(BodyId, BodyId)> {
        let owner = |collider: ColliderHandle| {
            self.entries
                .iter()
                .position(|entry| entry.live && entry.collider == collider)
                .map(BodyId)
        };
        let mut pairs: Vec<(BodyId, BodyId)> = self
            .narrow_phase
            .contact_pairs()
            .filter(|pair| pair.has_any_active_contact)
            .filter_map(|pair| {
                let (a, b) = (owner(pair.collider1)?, owner(pair.collider2)?);
                Some((a.min(b), a.max(b)))
            })
            .collect();
        pairs.sort();
        pairs.dedup();
        pairs
    }
}

#[cfg(test)]
mod tests;
