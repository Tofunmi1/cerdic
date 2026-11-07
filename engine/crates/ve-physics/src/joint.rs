//! Joints between bodies (ADR 0036): hinges, sliders, ball sockets, fixed welds and springs.
use rapier3d::na::{Isometry3, Point3, Translation3, UnitQuaternion, Vector3};
use rapier3d::prelude::{GenericJointBuilder, JointAxesMask, JointAxis, RigidBodyHandle};

use crate::{BodyId, Error, World};

/// Most joints in one world.
pub const MAX_JOINTS: usize = 512;

fn validate_joint(desc: &JointDesc) -> Result<(), Error> {
    let numbers = [
        desc.stiffness,
        desc.damping,
        desc.motor_speed,
        desc.motor_force,
        desc.rest_length,
    ];
    let mut all: Vec<f32> = numbers.to_vec();
    all.extend(desc.anchor);
    all.extend(desc.anchor_b);
    all.extend(desc.axis);
    if let Some(limits) = desc.limits {
        all.extend(limits);
        if limits[0] > limits[1] {
            return Err(Error::BadValue("joint limits must run from low to high"));
        }
    }
    if let Some(swing) = desc.swing {
        all.push(swing);
        if swing < 0.0 {
            return Err(Error::BadValue("a swing limit cannot be negative"));
        }
    }
    if let Some(twist) = desc.twist {
        all.extend(twist);
        if twist[0] > twist[1] {
            return Err(Error::BadValue("twist limits must run from low to high"));
        }
    }
    if !all.iter().all(|value| value.is_finite())
        || desc
            .anchor
            .iter()
            .chain(&desc.anchor_b)
            .any(|value| value.abs() > 1000.0)
    {
        return Err(Error::BadValue(
            "joint numbers must be finite and anchors within 1000 m",
        ));
    }
    if desc.stiffness < 0.0 || desc.damping < 0.0 || desc.motor_force < 0.0 {
        return Err(Error::BadValue(
            "stiffness, damping and motor force cannot be negative",
        ));
    }
    Ok(())
}

fn configure_joint_axis(
    mut builder: GenericJointBuilder,
    axis: JointAxis,
    desc: &JointDesc,
) -> GenericJointBuilder {
    if let Some(limits) = desc.limits {
        builder = builder.limits(axis, limits);
    }
    if desc.motor_force > 0.0 {
        builder = builder
            .motor_velocity(axis, desc.motor_speed, desc.damping.max(1.0))
            .motor_max_force(axis, desc.motor_force);
    } else if desc.damping > 0.0 {
        builder = builder
            .motor_velocity(axis, 0.0, desc.damping)
            .motor_max_force(axis, desc.damping * 5.0);
    }
    builder
}

/// How a joint lets its bodies move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JointKind {
    /// The two bodies stay rigidly together.
    Fixed,
    /// Turns about an axis through the anchor (a door, a wheel, a pendulum, a lever).
    Hinge,
    /// Slides along an axis (a drawer, a piston, a lift).
    Slider,
    /// Turns freely about the anchor in every direction (a rope link, a shoulder, a swing).
    Ball,
    /// Pulls the two anchor points toward a rest distance like a spring.
    Spring,
}

/// A joint to add. `b` of `None` joins `a` to the world itself.
#[derive(Clone, Copy, Debug)]
pub struct JointDesc {
    /// The first body.
    pub a: BodyId,
    /// The second body, or `None` for the fixed world.
    pub b: Option<BodyId>,
    /// How it moves.
    pub kind: JointKind,
    /// The joint's point on `a`, in world coordinates at the start.
    pub anchor: [f32; 3],
    /// Its point on `b` (springs only; others use `anchor` for both), in world coordinates at the start.
    pub anchor_b: [f32; 3],
    /// Hinge axis or slider direction in world coordinates (need not be unit length).
    pub axis: [f32; 3],
    /// Lowest and highest allowed angle (hinge, radians) or distance (slider, metres); `None` for unlimited.
    pub limits: Option<[f32; 2]>,
    /// Spring stiffness as acceleration per metre of stretch (about 5 is gentle, 50 stiff); the same for any mass.
    pub stiffness: f32,
    /// Damping: spring damping, or the resistance of a hinge or slider to moving (friction).
    pub damping: f32,
    /// A hinge or slider motor aiming for this speed (radians or metres per second); 0 with `motor_force` 0 means none.
    pub motor_speed: f32,
    /// The most force or torque the motor may use; 0 means no motor.
    pub motor_force: f32,
    /// Distance a spring rests at; a negative value means the distance between its anchors at the start.
    pub rest_length: f32,
    /// Ball joints only: the most the joint may swing away from straight along `axis`, in radians, in each of the two sideways directions
    /// (a cone of about this half-angle); `None` for no limit. Shoulders and hips are 1 to 1.5, a neck 0.5.
    pub swing: Option<f32>,
    /// Ball joints only: the lowest and highest twist about `axis`, in radians; `None` for free twist.
    pub twist: Option<[f32; 2]>,
}

impl Default for JointDesc {
    fn default() -> Self {
        Self {
            a: BodyId(0),
            b: None,
            kind: JointKind::Hinge,
            anchor: [0.0; 3],
            anchor_b: [0.0; 3],
            axis: [0.0, 1.0, 0.0],
            limits: None,
            stiffness: 50.0,
            damping: 1.0,
            motor_speed: 0.0,
            motor_force: 0.0,
            rest_length: -1.0,
            swing: None,
            twist: None,
        }
    }
}

fn point(p: [f32; 3]) -> Point3<f32> {
    Point3::new(p[0], p[1], p[2])
}

impl World {
    /// The body a joint end is on: the real one, or a fixed anchor body at the origin made on first use.
    fn joint_end(
        &mut self,
        end: Option<BodyId>,
    ) -> Result<(RigidBodyHandle, Isometry3<f32>), Error> {
        if let Some(id) = end {
            let handle = self.entry_handle(id)?;
            return Ok((handle, *self.bodies[handle].position()));
        }
        Ok((self.world_anchor(), Isometry3::identity()))
    }

    fn insert_joint(
        &mut self,
        desc: &JointDesc,
        first: RigidBodyHandle,
        second: RigidBodyHandle,
        frame_a: Isometry3<f32>,
        frame_b: Isometry3<f32>,
    ) {
        if desc.kind == JointKind::Spring {
            let rest = if desc.rest_length >= 0.0 {
                desc.rest_length
            } else {
                (point(desc.anchor) - point(desc.anchor_b)).norm()
            };
            self.springs.push(Spring {
                first,
                second,
                local_first: frame_a.translation.vector.into(),
                local_second: frame_b.translation.vector.into(),
                rest,
                stiffness: desc.stiffness,
                damping: desc.damping,
            });
            return;
        }
        let (mask, free) = match desc.kind {
            JointKind::Fixed => (JointAxesMask::LOCKED_FIXED_AXES, None),
            JointKind::Hinge => (JointAxesMask::LOCKED_REVOLUTE_AXES, Some(JointAxis::AngX)),
            JointKind::Slider => (JointAxesMask::LOCKED_PRISMATIC_AXES, Some(JointAxis::LinX)),
            JointKind::Ball | JointKind::Spring => (JointAxesMask::LOCKED_SPHERICAL_AXES, None),
        };
        let mut builder = GenericJointBuilder::new(mask)
            .contacts_enabled(false)
            .local_frame1(frame_a)
            .local_frame2(frame_b);
        if let Some(axis) = free {
            builder = configure_joint_axis(builder, axis, desc);
        }
        if desc.kind == JointKind::Ball {
            // The joint frame has local X along `axis`: twist is about X, swing about Y and Z (a cone, approximated by two limits).
            if let Some(swing) = desc.swing {
                builder = builder
                    .limits(JointAxis::AngY, [-swing, swing])
                    .limits(JointAxis::AngZ, [-swing, swing]);
            }
            if let Some(twist) = desc.twist {
                builder = builder.limits(JointAxis::AngX, twist);
            }
            if desc.damping > 0.0 {
                for axis in [JointAxis::AngX, JointAxis::AngY, JointAxis::AngZ] {
                    builder = builder
                        .motor_velocity(axis, 0.0, desc.damping)
                        .motor_max_force(axis, desc.damping * 5.0);
                }
            }
        }
        let handle = self
            .impulse_joints
            .insert(first, second, builder.build(), true);
        self.joints.push(handle);
    }

    /// Joins two bodies (or one body and the world).
    ///
    /// # Errors
    /// [`Error::NoSuchBody`], [`Error::BadValue`] for numbers that are not finite, a zero axis, an anchor farther than 1,000 m,
    /// limits out of order, or more than [`MAX_JOINTS`] joints.
    pub fn add_joint(&mut self, desc: &JointDesc) -> Result<(), Error> {
        if self.joint_count() >= MAX_JOINTS {
            return Err(Error::BadValue("a world holds at most 512 joints"));
        }
        validate_joint(desc)?;
        let (first, pose_a) = self.joint_end(Some(desc.a))?;
        let (second, pose_b) = self.joint_end(desc.b)?;
        let world_axis = Vector3::new(desc.axis[0], desc.axis[1], desc.axis[2]);
        if matches!(desc.kind, JointKind::Hinge | JointKind::Slider) && world_axis.norm() < 1.0e-6 {
            return Err(Error::BadValue("a hinge or slider needs a non-zero axis"));
        }
        // The joint frame: origin at the anchor, local X along the axis, expressed in each body's own frame.
        let facing = UnitQuaternion::rotation_between(&Vector3::x(), &world_axis)
            .unwrap_or_else(UnitQuaternion::identity);
        let frame = |pose: &Isometry3<f32>, at: [f32; 3]| {
            let local_origin = pose.inverse_transform_point(&point(at));
            let local_turn = pose.rotation.inverse() * facing;
            Isometry3::from_parts(
                Translation3::new(local_origin.x, local_origin.y, local_origin.z),
                local_turn,
            )
        };
        let (frame_a, frame_b) = (
            frame(&pose_a, desc.anchor),
            frame(
                &pose_b,
                if desc.kind == JointKind::Spring {
                    desc.anchor_b
                } else {
                    desc.anchor
                },
            ),
        );
        self.insert_joint(desc, first, second, frame_a, frame_b);
        Ok(())
    }

    /// How many joints the world holds (springs included).
    #[must_use]
    pub fn joint_count(&self) -> usize {
        self.joints.len() + self.springs.len()
    }

    /// Pulls every spring's two anchor points toward its rest distance, as a force per unit of reduced mass, with damping
    /// along the spring. Called at the start of each step.
    pub(crate) fn apply_springs(&mut self) {
        for spring in self.springs.clone() {
            let state = |handle: RigidBodyHandle, local: Point3<f32>| {
                let body = &self.bodies[handle];
                let world = body.position() * local;
                (
                    world,
                    body.velocity_at_point(&world),
                    if body.is_dynamic() {
                        Some(body.mass())
                    } else {
                        None
                    },
                )
            };
            let ((from, from_velocity, from_mass), (to, to_velocity, to_mass)) = (
                state(spring.first, spring.local_first),
                state(spring.second, spring.local_second),
            );
            let along = to - from;
            let length = along.norm();
            if length < 1.0e-6 {
                continue;
            }
            let direction = along / length;
            let speed = (to_velocity - from_velocity).dot(&direction);
            let reduced = match (from_mass, to_mass) {
                (Some(a), Some(b)) => a * b / (a + b),
                (Some(m), None) | (None, Some(m)) => m,
                (None, None) => continue,
            };
            // Positive when stretched: the anchors are pulled together.
            let force =
                reduced * (spring.stiffness * (length - spring.rest) + spring.damping * speed);
            let impulse = direction * (force * crate::STEP);
            if from_mass.is_some() {
                self.bodies[spring.first].apply_impulse_at_point(impulse, from, true);
            }
            if to_mass.is_some() {
                self.bodies[spring.second].apply_impulse_at_point(-impulse, to, true);
            }
        }
    }
}

/// A spring between two points on two bodies.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Spring {
    pub(crate) first: RigidBodyHandle,
    pub(crate) second: RigidBodyHandle,
    local_first: Point3<f32>,
    local_second: Point3<f32>,
    rest: f32,
    stiffness: f32,
    damping: f32,
}

#[cfg(test)]
mod tests;
