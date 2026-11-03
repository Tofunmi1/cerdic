//! The forces on a particle, each an acceleration over one step, and the curl-noise turbulence field.
use ve_world::noise::{Basis, Noise};

use super::Force;

type V = [f32; 3];

fn add(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(a: V, s: f32) -> V {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn dot(a: V, b: V) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V, b: V) -> V {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn length(a: V) -> f32 {
    dot(a, a).sqrt()
}

/// Whether a force's numbers make sense.
pub(super) fn valid(force: &Force) -> bool {
    let ok = |values: &[f32]| values.iter().all(|v| v.is_finite());
    match force {
        Force::Gravity(g) => ok(g),
        Force::Wind { velocity, strength } => ok(velocity) && ok(&[*strength]) && *strength >= 0.0,
        Force::Drag(c) => c.is_finite() && *c >= 0.0,
        Force::Vortex {
            centre,
            axis,
            strength,
            radius,
        } => ok(centre) && ok(axis) && length(*axis) > 0.0 && strength.is_finite() && *radius > 0.0 && radius.is_finite(),
        Force::Attractor {
            centre,
            strength,
            radius,
        } => ok(centre) && strength.is_finite() && *radius > 0.0 && radius.is_finite(),
        Force::Turbulence {
            strength,
            scale,
            speed,
        } => ok(&[*strength, *speed]) && *scale > 0.0 && scale.is_finite(),
    }
}

/// The noise behind each component of the turbulence's vector potential: simplex, so no grid-aligned streaks.
fn potential(seed: u32, point: [f64; 3], time: f64) -> f64 {
    Noise {
        basis: Basis::Simplex,
        seed,
        frequency: 1.0,
        ..Noise::default()
    }
    .sample4(point[0], point[1], point[2], time)
}

/// A swirling vector field with no sources or sinks: the curl of a vector potential made of three simplex noises.
/// `strength` scales it, `scale` is features per metre, `time` moves the pattern (the fourth noise coordinate).
#[must_use]
pub fn turbulence_field(strength: f32, scale: f32, time: f32, position: [f32; 3]) -> [f32; 3] {
    const STEP: f64 = 0.01;
    let p = position.map(|c| f64::from(c) * f64::from(scale));
    let t = f64::from(time);
    // d/dx of component `c` of the potential, by central difference, in noise units.
    let slope = |component: u32, axis: usize| {
        let (mut hi, mut lo) = (p, p);
        hi[axis] += STEP;
        lo[axis] -= STEP;
        (potential(0x51ED_0000 + component, hi, t) - potential(0x51ED_0000 + component, lo, t)) / (2.0 * STEP)
    };
    let curl = [
        slope(2, 1) - slope(1, 2),
        slope(0, 2) - slope(2, 0),
        slope(1, 0) - slope(0, 1),
    ];
    #[expect(clippy::cast_possible_truncation, reason = "the field is a few metres per second squared")]
    let out = curl.map(|c| (c * f64::from(strength) * f64::from(scale)) as f32);
    out
}

/// Applies one force to a velocity over `dt`; `time` is the system's clock (for turbulence).
pub(super) fn apply(force: &Force, position: V, velocity: V, dt: f32, time: f32) -> V {
    match *force {
        Force::Gravity(g) => add(velocity, scale(g, dt)),
        Force::Wind {
            velocity: wind,
            strength,
        } => {
            // Implicit: stable however hard the drag, and it converges on the wind speed.
            let k = strength * dt;
            scale(add(velocity, scale(wind, k)), 1.0 / (1.0 + k))
        }
        Force::Drag(c) => scale(velocity, 1.0 / (1.0 + c * dt)),
        Force::Vortex {
            centre,
            axis,
            strength,
            radius,
        } => {
            let axis = scale(axis, 1.0 / length(axis));
            let from_centre = sub(position, centre);
            let radial = sub(from_centre, scale(axis, dot(from_centre, axis)));
            let distance = length(radial);
            if distance < 1.0e-4 || distance > radius {
                return velocity;
            }
            let tangent = scale(cross(axis, radial), 1.0 / distance);
            add(velocity, scale(tangent, strength * (1.0 - distance / radius) * dt))
        }
        Force::Attractor {
            centre,
            strength,
            radius,
        } => {
            let to = sub(centre, position);
            let distance = length(to);
            if distance > radius {
                return velocity;
            }
            // Softened so a particle sitting on the centre is not flung out by a division by zero.
            let direction = scale(to, 1.0 / (distance * distance + 0.01).sqrt());
            add(velocity, scale(direction, strength * (1.0 - distance / radius) * dt))
        }
        Force::Turbulence {
            strength,
            scale: features,
            speed,
        } => add(
            velocity,
            scale(turbulence_field(strength, features, time * speed, position), dt),
        ),
    }
}
