//! Ragdolls: a skeleton of capsule bones joined by ball and hinge joints with limits, built into a [`World`] in one call.
//!
//! Plan W5, `docs/plans/engine-libraries-adoption.md`. The shape of the model (capsule per bone, swing-twist limits on shoulders and hips, hinge limits on
//! elbows and knees, no collision between a body's own bones) follows Jolt's ragdoll settings (MIT); the code is ours on Rapier.
use crate::{BodyDesc, BodyId, Error, JointDesc, JointKind, Kind, Layers, Shape, World};

/// Most bones in one ragdoll.
pub const MAX_BONES: usize = 64;
/// The collision group every ragdoll body belongs to, so a ragdoll can ignore its own bones.
const RAGDOLL_GROUP: u32 = 1 << 1;

/// How a bone is joined to its parent at its head.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BoneJoint {
    /// Welded to the parent (the root bone has no joint, whatever this says).
    Fixed,
    /// A socket: swing up to `swing` radians away from the bone's rest direction (a cone), twist within `twist`.
    Ball {
        /// Half-angle of the cone, radians.
        swing: f32,
        /// Lowest and highest twist about the bone, radians.
        twist: [f32; 2],
    },
    /// A hinge about a world axis (an elbow, a knee), within `limits` radians from the rest pose.
    Hinge {
        /// Hinge axis in world coordinates at rest.
        axis: [f32; 3],
        /// Lowest and highest angle.
        limits: [f32; 2],
    },
}

/// One bone at rest, in world coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bone {
    /// The index of the parent bone, which must come earlier in the list; `None` for the root.
    pub parent: Option<usize>,
    /// Where the bone starts (its joint).
    pub head: [f32; 3],
    /// Where it ends.
    pub tail: [f32; 3],
    /// Thickness of the capsule that stands for it, metres.
    pub radius: f32,
    /// How it joins its parent.
    pub joint: BoneJoint,
}

/// A skeleton to turn into bodies.
#[derive(Clone, Debug)]
pub struct RagdollDesc {
    /// The bones, parents first.
    pub bones: Vec<Bone>,
    /// Mass per cubic metre (a person is about 400 to 1000).
    pub density: f32,
    /// Sliding friction of every bone.
    pub friction: f32,
    /// Whether bones of this ragdoll collide with each other (neighbours never do).
    pub self_collide: bool,
    /// Resistance of the joints to moving: limp (0) to stiff.
    pub joint_damping: f32,
}

/// The bodies of a built ragdoll, one per bone in the same order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ragdoll {
    /// A body per bone.
    pub bodies: Vec<BodyId>,
}

fn finite(values: &[f32]) -> bool {
    values.iter().all(|v| v.is_finite())
}

fn length(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn validate(desc: &RagdollDesc) -> Result<(), Error> {
    if desc.bones.is_empty() || desc.bones.len() > MAX_BONES {
        return Err(Error::BadValue("a ragdoll needs 1 to 64 bones"));
    }
    if !(finite(&[desc.density, desc.friction, desc.joint_damping]) && desc.density > 0.0 && desc.friction >= 0.0 && desc.joint_damping >= 0.0) {
        return Err(Error::BadValue("density must be above 0; friction and damping not negative"));
    }
    for (index, bone) in desc.bones.iter().enumerate() {
        if !(finite(&bone.head) && finite(&bone.tail) && finite(&[bone.radius])) {
            return Err(Error::BadValue("bone numbers must be finite"));
        }
        if bone.radius <= 0.0 || bone.radius > 5.0 {
            return Err(Error::BadValue("a bone needs a radius above 0 and at most 5 m"));
        }
        let along = [bone.tail[0] - bone.head[0], bone.tail[1] - bone.head[1], bone.tail[2] - bone.head[2]];
        if length(along) < 1.0e-3 {
            return Err(Error::BadValue("a bone needs a length of at least 1 mm"));
        }
        if bone.parent.is_some_and(|parent| parent >= index) {
            return Err(Error::BadValue("a bone's parent must come before it in the list"));
        }
        if let BoneJoint::Hinge { axis, limits } = bone.joint
            && (!finite(&axis) || !finite(&limits) || length(axis) < 1.0e-6 || limits[0] > limits[1])
        {
            return Err(Error::BadValue("a hinge needs a non-zero axis and limits from low to high"));
        }
        if let BoneJoint::Ball { swing, twist } = bone.joint
            && (!finite(&[swing, twist[0], twist[1]]) || swing < 0.0 || twist[0] > twist[1])
        {
            return Err(Error::BadValue("a ball joint needs a swing of 0 or more and twist limits from low to high"));
        }
    }
    Ok(())
}

/// Where a bone's body starts: its centre (the middle of the bone) and its rotation (+Y along the bone).
#[must_use]
pub fn rest_frame(bone: &Bone) -> ([f32; 3], [f32; 4]) {
    let along = [bone.tail[0] - bone.head[0], bone.tail[1] - bone.head[1], bone.tail[2] - bone.head[2]];
    let total = length(along).max(1.0e-9);
    ([0, 1, 2].map(|a| (bone.head[a] + bone.tail[a]) * 0.5), y_to(along.map(|c| c / total)))
}

fn multiply(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

/// The pose (column-major 4x4) of a bone part whose body has moved from `(rest_position, rest_rotation)` to `(position, rotation)`: the motion
/// that carries the part's authored mesh along with its body (turn about the bone's middle, then move). The identity when the body has not moved.
#[must_use]
pub fn bone_pose(rest_position: [f32; 3], rest_rotation: [f32; 4], position: [f32; 3], rotation: [f32; 4]) -> [f32; 16] {
    let rest_inverse = [-rest_rotation[0], -rest_rotation[1], -rest_rotation[2], rest_rotation[3]];
    let turn = multiply(rotation, rest_inverse);
    let norm = turn.iter().map(|c| c * c).sum::<f32>().sqrt().max(1.0e-9);
    let [x, y, z, w] = turn.map(|c| c / norm);
    // Rotation matrix columns, then the translation that turns about the rest centre and lands on the new one.
    let c0 = [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + z * w), 2.0 * (x * z - y * w)];
    let c1 = [2.0 * (x * y - z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + x * w)];
    let c2 = [2.0 * (x * z + y * w), 2.0 * (y * z - x * w), 1.0 - 2.0 * (x * x + y * y)];
    let turned = [0, 1, 2].map(|r| c0[r] * rest_position[0] + c1[r] * rest_position[1] + c2[r] * rest_position[2]);
    let shift = [0, 1, 2].map(|a| position[a] - turned[a]);
    [
        c0[0], c0[1], c0[2], 0.0, c1[0], c1[1], c1[2], 0.0, c2[0], c2[1], c2[2], 0.0, shift[0], shift[1], shift[2], 1.0,
    ]
}

/// The rotation (x, y, z, w) that carries +Y onto the unit vector `to`, without trigonometry.
fn y_to(to: [f32; 3]) -> [f32; 4] {
    let dot = to[1];
    if dot < -1.0 + 1.0e-6 {
        return [1.0, 0.0, 0.0, 0.0];
    }
    // cross(Y, to) = (to.z, 0, -to.x)
    let (x, y, z, w) = (to[2], 0.0, -to[0], 1.0 + dot);
    let norm = (x * x + y * y + z * z + w * w).sqrt();
    [x / norm, y / norm, z / norm, w / norm]
}

/// Builds the bodies and joints of a skeleton into `world`. The skeleton is checked completely first, so a refused one adds nothing.
///
/// # Errors
/// [`Error::BadValue`] for a skeleton that cannot work (see [`MAX_BONES`], parents before children, finite numbers, a radius, a length,
/// non-zero hinge axes) and any error the world gives for adding a body or joint (for example too many bodies).
pub fn build(world: &mut World, desc: &RagdollDesc) -> Result<Ragdoll, Error> {
    validate(desc)?;
    let group = Layers {
        member: RAGDOLL_GROUP,
        mask: if desc.self_collide { u32::MAX } else { !RAGDOLL_GROUP },
    };
    let mut bodies = Vec::with_capacity(desc.bones.len());
    for bone in &desc.bones {
        let along = [bone.tail[0] - bone.head[0], bone.tail[1] - bone.head[1], bone.tail[2] - bone.head[2]];
        let total = length(along);
        let direction = along.map(|c| c / total);
        let id = world.add(&BodyDesc {
            kind: Kind::Dynamic,
            shape: Shape::Capsule {
                half_height: (total * 0.5 - bone.radius).max(0.0),
                radius: bone.radius,
            },
            position: [0, 1, 2].map(|a| (bone.head[a] + bone.tail[a]) * 0.5),
            rotation: y_to(direction),
            density: desc.density,
            friction: desc.friction,
            restitution: 0.0,
            linear_damping: 0.05,
            angular_damping: 0.3,
            layers: group,
            sensor: false,
        })?;
        bodies.push(id);
    }
    for (index, bone) in desc.bones.iter().enumerate() {
        let Some(parent) = bone.parent else {
            continue;
        };
        let along = [bone.tail[0] - bone.head[0], bone.tail[1] - bone.head[1], bone.tail[2] - bone.head[2]];
        let base = JointDesc {
            a: bodies[parent],
            b: Some(bodies[index]),
            anchor: bone.head,
            damping: desc.joint_damping,
            ..JointDesc::default()
        };
        let joint = match bone.joint {
            BoneJoint::Fixed => JointDesc {
                kind: JointKind::Fixed,
                ..base
            },
            BoneJoint::Ball { swing, twist } => JointDesc {
                kind: JointKind::Ball,
                axis: along,
                swing: Some(swing),
                twist: Some(twist),
                ..base
            },
            BoneJoint::Hinge { axis, limits } => JointDesc {
                kind: JointKind::Hinge,
                axis,
                limits: Some(limits),
                ..base
            },
        };
        world.add_joint(&joint)?;
    }
    Ok(Ragdoll { bodies })
}
