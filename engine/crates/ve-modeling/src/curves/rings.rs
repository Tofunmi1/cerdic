//! The cross-sections of a round tube along a polyline: where each ring faces, its two axes, and how a bend stretches it.
//!
//! The axes are rotation-minimising (parallel-transported) frames, so a tube does not twist as it winds; on a closed curve the
//! twist left over after one lap is spread evenly over the rings so the last one meets the first. At a bend the ring lies in
//! the plane that bisects the turn and is stretched along the bend so the tube keeps its width (a mitre).
use super::vec3::{add, cross, dot, length, normalize, scale, sub};

type P3 = [f64; 3];

/// The sharpest turn at one point, in degrees; beyond it the tube folds back on itself.
pub(super) const MAX_TURN_DEGREES: f64 = 150.0;

/// One cross-section of the tube. `normal` and `binormal` are unit axes in the ring's plane, which is perpendicular to
/// `tangent`; the ring is stretched along `miter` (a unit vector in that plane, along the bend) by `stretch`.
pub(super) struct Ring {
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the frame tests read the tangent; the mesh builder needs only the offsets"
        )
    )]
    pub(super) tangent: P3,
    pub(super) normal: P3,
    pub(super) binormal: P3,
    pub(super) miter: P3,
    pub(super) stretch: f64,
}

impl Ring {
    /// The point of the ring at `angle` (radians around the tangent) and `radius`, relative to the ring's centre.
    pub(super) fn offset(&self, angle: f64, radius: f64) -> P3 {
        let around = add(
            scale(self.normal, angle.cos()),
            scale(self.binormal, angle.sin()),
        );
        let along = scale(self.miter, (self.stretch - 1.0) * dot(around, self.miter));
        scale(add(around, along), radius)
    }
}

/// A turn too sharp for a tube to follow (a hairpin, or more than [`MAX_TURN_DEGREES`] at one point).
#[derive(Debug, PartialEq, Eq)]
pub(super) struct TooSharp;

/// Turns `v` about the unit `axis` by `angle` (Rodrigues' formula).
fn rotate(v: P3, axis: P3, angle: f64) -> P3 {
    let (sin, cos) = angle.sin_cos();
    add(
        add(scale(v, cos), scale(cross(axis, v), sin)),
        scale(axis, dot(axis, v) * (1.0 - cos)),
    )
}

/// The smallest rotation taking the unit direction `from` to `to`, applied to `v`: how a frame is carried along a bend.
pub(super) fn transport(v: P3, from: P3, to: P3) -> P3 {
    let Some(axis) = normalize(cross(from, to)) else {
        return v;
    };
    rotate(v, axis, dot(from, to).clamp(-1.0, 1.0).acos())
}

/// The direction of the segment from `a` to `b`.
fn direction(a: P3, b: P3) -> P3 {
    normalize(sub(b, a)).unwrap_or([0.0, 0.0, 1.0])
}

/// The rings of a polyline with at least two points (three if closed) and no repeated points.
#[expect(clippy::cast_precision_loss, reason = "a ring index is far below 2^52")]
pub(super) fn rings(path: &[P3], closed: bool) -> Result<Vec<Ring>, TooSharp> {
    let count = path.len();
    let before = |i: usize| path[(i + count - 1) % count];
    let after = |i: usize| path[(i + 1) % count];
    let is_corner = |i: usize| closed || (i > 0 && i + 1 < count);
    let mut tangents = Vec::with_capacity(count);
    let mut turns = Vec::with_capacity(count);
    for (i, &point) in path.iter().enumerate() {
        if !is_corner(i) {
            let end = if i == 0 {
                direction(point, after(i))
            } else {
                direction(before(i), point)
            };
            tangents.push(end);
            turns.push(0.0);
            continue;
        }
        let (a, b) = (direction(before(i), point), direction(point, after(i)));
        let turn = dot(a, b).clamp(-1.0, 1.0).acos();
        let bisector = normalize(add(a, b)).ok_or(TooSharp)?;
        if turn.to_degrees() > MAX_TURN_DEGREES {
            return Err(TooSharp);
        }
        tangents.push(bisector);
        turns.push(turn);
    }

    // The first normal is any vector perpendicular to the first tangent, chosen the same way every time.
    let seed = if tangents[0][0].abs() < 0.8 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let mut normal = normalize(cross(tangents[0], seed)).unwrap_or([1.0, 0.0, 0.0]);
    let mut normals = vec![normal];
    for pair in tangents.windows(2) {
        normal = transport(normal, pair[0], pair[1]);
        normal = normalize(sub(normal, scale(pair[1], dot(normal, pair[1])))).unwrap_or(normal);
        normals.push(normal);
    }
    if closed {
        // Carry the last normal on to the first ring; what is left over is spread evenly over the rings.
        let last = tangents[count - 1];
        let carried = transport(normals[count - 1], last, tangents[0]);
        let residual = dot(cross(carried, normals[0]), tangents[0]).atan2(dot(carried, normals[0]));
        for (i, ring_normal) in normals.iter_mut().enumerate() {
            let share = residual * i as f64 / count as f64;
            *ring_normal = rotate(*ring_normal, tangents[i], share);
        }
    }

    Ok((0..count)
        .map(|i| {
            let (tangent, normal) = (tangents[i], normals[i]);
            let binormal = normalize(cross(tangent, normal)).unwrap_or(normal);
            // The ring stretches along the direction that is in the plane of the bend and perpendicular to the bisector.
            let (a, b) = (direction(before(i), path[i]), direction(path[i], after(i)));
            let bend = if is_corner(i) {
                normalize(sub(b, a))
            } else {
                None
            };
            Ring {
                tangent,
                normal,
                binormal,
                miter: bend.unwrap_or(normal),
                stretch: if bend.is_some() {
                    (1.0 / (turns[i] * 0.5).cos()).min(MAX_STRETCH)
                } else {
                    1.0
                },
            }
        })
        .collect())
}

/// Most a ring is stretched at a bend.
const MAX_STRETCH: f64 = 4.0;

/// The distance of the segment between two path points, for callers that need arc lengths.
pub(super) fn segment_length(a: P3, b: P3) -> f64 {
    length(sub(b, a))
}

#[cfg(test)]
#[path = "rings_tests.rs"]
mod tests;
