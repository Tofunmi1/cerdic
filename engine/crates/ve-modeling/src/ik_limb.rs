//! Analytic limb solvers: a two-bone chain (arm, leg) with pole vector, twist, soften and weight, and an aim solver (head, eye, turret).
//!
//! The two-bone soften curve and the weight convention follow ozz-animation's
//! `IKTwoBoneJob` (MIT, Guillaume Blanc); the soften curve differs (see `soften`). The geometry here is our own, in world space: the elbow position comes
//! from the law of cosines, the pole picks the side, and rotations are derived from where the bones must end up.
//!
//! Arithmetic and `sqrt` only, except `twist` which is the caller's angle (one `sin_cos`), so results are the same on every target.
//! Everything is in `f64`; inputs that cannot work (zero-length bones, a target on the root, non-finite numbers) give the identity.
use glam::{DMat3, DQuat, DVec3};

/// Below this a length or a vector counts as zero.
const TINY: f64 = 1.0e-9;

/// The current world positions of a three-joint chain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TwoBone {
    /// The root joint (shoulder, hip).
    pub start: DVec3,
    /// The middle joint (elbow, knee).
    pub mid: DVec3,
    /// The tip (wrist, ankle).
    pub end: DVec3,
}

/// What the chain should do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TwoBoneGoal {
    /// Where the tip should be (world).
    pub target: DVec3,
    /// A world direction the middle joint bends towards (the knee points forward, the elbow back). Zero keeps the current bend side.
    pub pole: DVec3,
    /// `0..=1`: how much of the chain's length, at the far end, the arm eases in over instead of snapping straight. `0` is none.
    pub soften: f64,
    /// Extra swing of the middle joint around the root-to-target line, in radians.
    pub twist: f64,
    /// `0..=1`: how much of the correction to apply.
    pub weight: f64,
}

/// The corrections for a two-bone chain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TwoBoneSolution {
    /// A world rotation about `start`, applied to the whole chain.
    pub start: DQuat,
    /// A further world rotation about the (already moved) middle joint, applied to the lower bone only.
    pub mid: DQuat,
    /// The tip reached the target exactly (not softened, not out of range) at full weight.
    pub reached: bool,
}

impl TwoBoneSolution {
    /// The middle joint and the tip after the corrections.
    #[must_use]
    pub fn pose(&self, chain: &TwoBone) -> (DVec3, DVec3) {
        let mid = chain.start + self.start * (chain.mid - chain.start);
        let end = mid + (self.mid * self.start) * (chain.end - chain.mid);
        (mid, end)
    }
}

const NOTHING: TwoBoneSolution = TwoBoneSolution {
    start: DQuat::IDENTITY,
    mid: DQuat::IDENTITY,
    reached: false,
};

/// The soft landing: the distance the tip is allowed to reach for a target `distance` away. Equal to `distance` up to the start of the soft
/// zone, then `alpha / (1 + alpha)` of the zone: slope 1 where it starts, never past the target, never quite full stretch.
/// (ozz's quartic version has slope 4/3 there, so it overshoots the target a little before easing in.)
/// Returns the allowed distance and whether it was shortened.
fn soften(distance: f64, reach: f64, difference: f64, amount: f64) -> (f64, bool) {
    let soft_start = reach * (1.0 - amount.clamp(0.0, 1.0));
    let span = reach - soft_start;
    if distance > soft_start && span > TINY && distance > difference {
        let alpha = (distance - soft_start) / span;
        (soft_start + span * alpha / (1.0 + alpha), true)
    } else {
        (distance, false)
    }
}

/// A unit vector perpendicular to `axis` (unit) that leans towards `hint`, or failing that towards `fallback`, or any.
fn sideways(axis: DVec3, hint: DVec3, fallback: DVec3) -> DVec3 {
    for candidate in [hint, fallback] {
        let flat = candidate - axis * candidate.dot(axis);
        if flat.is_finite() && flat.length() > TINY {
            return flat.normalize();
        }
    }
    let other = if axis.x.abs() < 0.6 {
        DVec3::X
    } else {
        DVec3::Y
    };
    (other - axis * other.dot(axis)).normalize()
}

/// The world rotation that carries unit vector `from` onto unit vector `to`.
fn arc(from: DVec3, to: DVec3) -> DQuat {
    DQuat::from_rotation_arc(from, to).normalize()
}

fn blend(q: DQuat, weight: f64) -> DQuat {
    if weight >= 1.0 {
        return q;
    }
    // Shortest path, normalised lerp: arithmetic only.
    let q = if q.w < 0.0 { -q } else { q };
    DQuat::IDENTITY.lerp(q, weight).normalize()
}

/// Solves a two-bone chain: the corrections that bring the tip to the goal, with the middle joint on the pole's side.
#[must_use]
pub fn two_bone(chain: &TwoBone, goal: &TwoBoneGoal) -> TwoBoneSolution {
    let weight = goal.weight.clamp(0.0, 1.0);
    let upper = chain.mid - chain.start;
    let lower = chain.end - chain.mid;
    let to_target = goal.target - chain.start;
    let (a, b, distance) = (upper.length(), lower.length(), to_target.length());
    let finite = upper.is_finite() && lower.is_finite() && to_target.is_finite();
    if weight <= 0.0 || !finite || a < TINY || b < TINY || distance < TINY {
        return NOTHING;
    }
    let reach = a + b;
    let difference = (a - b).abs();
    let (softened, shortened) = soften(distance, reach, difference, goal.soften);
    let direction = to_target / distance;
    let used = softened.clamp(difference.max(TINY * reach), reach);

    // Law of cosines: the middle joint is `along` the line to the target and `across` it, towards the pole.
    let along = (used * used + a * a - b * b) / (2.0 * used);
    let across = (a * a - along * along).max(0.0).sqrt();
    let bend = sideways(direction, goal.pole, upper);
    let bend = if goal.twist == 0.0 {
        bend
    } else {
        DQuat::from_axis_angle(direction, goal.twist) * bend
    };
    let new_mid = direction * along + bend * across;
    let new_end = direction * used;

    // The upper bone swings onto its new direction, then rolls so the bend plane stays the same plane of the bones.
    let upper_unit = upper / a;
    let new_upper = new_mid / a;
    let mut start_turn = arc(upper_unit, new_upper);
    let old_normal = upper.cross(lower);
    let new_normal = new_mid.cross(new_end - new_mid);
    if old_normal.length() > TINY * a * b && new_normal.length() > TINY * a * b {
        let carried = (start_turn * old_normal).normalize();
        let wanted = new_normal.normalize();
        let roll = arc_about(new_upper, carried, wanted);
        start_turn = roll * start_turn;
    }
    // The lower bone swings onto its new direction.
    let carried_lower = (start_turn * lower) / b;
    let new_lower = (new_end - new_mid) / b;
    let mid_turn = arc(carried_lower, new_lower);

    TwoBoneSolution {
        start: blend(start_turn, weight),
        mid: blend(mid_turn, weight),
        reached: weight >= 1.0 && distance <= reach && distance > difference && !shortened,
    }
}

/// The rotation about `axis` (unit) that carries `from` onto `to` (both unit and perpendicular to `axis`), by the shorter way.
fn arc_about(axis: DVec3, from: DVec3, to: DVec3) -> DQuat {
    let across = from.cross(to);
    let cosine = from.dot(to);
    if cosine < -1.0 + TINY {
        return DQuat::from_xyzw(axis.x, axis.y, axis.z, 0.0);
    }
    // Half-angle construction: (1 + cos, from x to) normalised is the rotation by the angle between them.
    let projected = axis * across.dot(axis);
    DQuat::from_xyzw(projected.x, projected.y, projected.z, 1.0 + cosine).normalize()
}

/// A joint that should look at something.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aim {
    /// The joint's world position.
    pub position: DVec3,
    /// The joint's current world rotation.
    pub rotation: DQuat,
    /// The joint's local axis that should point at the target.
    pub forward: DVec3,
    /// The joint's local axis that should point towards the pole.
    pub up: DVec3,
}

/// What the joint should look at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AimGoal {
    /// The world point to look at.
    pub target: DVec3,
    /// A world direction for `up` to lean towards. Zero (or parallel to the view line) keeps the current up.
    pub pole: DVec3,
    /// Extra roll around the view line, in radians.
    pub twist: f64,
    /// `0..=1`: how much of the correction to apply.
    pub weight: f64,
}

/// The correction for an aim.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AimSolution {
    /// A world rotation to apply on top of the joint's rotation (`correction * rotation`).
    pub correction: DQuat,
    /// The joint is looking at the target at full weight.
    pub reached: bool,
}

/// Turns a joint so its `forward` axis points at the target and its `up` axis leans towards the pole.
#[must_use]
pub fn aim(joint: &Aim, goal: &AimGoal) -> AimSolution {
    let nothing = AimSolution {
        correction: DQuat::IDENTITY,
        reached: false,
    };
    let weight = goal.weight.clamp(0.0, 1.0);
    let to_target = goal.target - joint.position;
    let finite = to_target.is_finite() && joint.forward.is_finite() && joint.up.is_finite();
    if weight <= 0.0 || !finite || to_target.length() < TINY || joint.forward.length() < TINY {
        return nothing;
    }
    let view = to_target.normalize();
    let forward = joint.forward.normalize();
    let up = sideways(forward, joint.up, DVec3::ZERO);

    // The joint's own frame (forward, up, right-handed third axis) and the frame it should end up in.
    let local = DMat3::from_cols(forward, up, forward.cross(up));
    let current_up = joint.rotation * up;
    let wanted_up = sideways(view, goal.pole, current_up);
    let wanted_up = if goal.twist == 0.0 {
        wanted_up
    } else {
        DQuat::from_axis_angle(view, goal.twist) * wanted_up
    };
    let wanted = DMat3::from_cols(view, wanted_up, view.cross(wanted_up));
    let turn = DQuat::from_mat3(&(wanted * local.transpose())).normalize();
    let correction = turn * joint.rotation.inverse();
    AimSolution {
        correction: blend(correction.normalize(), weight),
        reached: weight >= 1.0,
    }
}
