//! Small deterministic math and boundary helpers for the PBF reference solver.
use super::{Error, FloorGrid, MAX_GRID_COORDINATE, Options, Particle};

pub(super) fn validate_options(o: Options) -> Result<(), Error> {
    if !o.min.iter().chain(&o.max).all(|v| v.is_finite())
        || (0..3).any(|axis| o.min[axis] >= o.max[axis])
    {
        return Err(Error::Parameter("bounds"));
    }
    if !o.smoothing_radius.is_finite() || o.smoothing_radius <= 0.0 {
        return Err(Error::Parameter("smoothing radius"));
    }
    if (0..3).any(|axis| {
        let cells = (o.max[axis] - o.min[axis]) / o.smoothing_radius;
        !cells.is_finite() || cells > MAX_GRID_COORDINATE
    }) {
        return Err(Error::Parameter("bounds-to-cell ratio"));
    }
    if !o.rest_density.is_finite() || o.rest_density <= 0.0 {
        return Err(Error::Parameter("rest density"));
    }
    if !o.particle_mass.is_finite() || o.particle_mass <= 0.0 {
        return Err(Error::Parameter("particle mass"));
    }
    if !o.gravity.iter().all(|v| v.is_finite()) {
        return Err(Error::Parameter("gravity"));
    }
    if !o.damping.is_finite() || o.damping < 0.0 {
        return Err(Error::Parameter("damping"));
    }
    if !(1..=8).contains(&o.iterations) {
        return Err(Error::Parameter("iterations"));
    }
    Ok(())
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "validate_options bounds every coordinate divided by smoothing radius below i32::MAX"
)]
pub(super) fn cell(p: [f32; 3], origin: [f32; 3], size: f32) -> [i32; 3] {
    [
        ((p[0] - origin[0]) / size).floor() as i32,
        ((p[1] - origin[1]) / size).floor() as i32,
        ((p[2] - origin[2]) / size).floor() as i32,
    ]
}

pub(super) fn inside(p: [f32; 3], o: Options) -> bool {
    (0..3).all(|axis| p[axis] >= o.min[axis] && p[axis] <= o.max[axis])
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "floor dimensions and particle positions are bounded"
)]
pub(super) fn collide(p: &mut Particle, o: Options, floor: Option<&FloorGrid>) {
    for axis in 0..3 {
        if p.position[axis] < o.min[axis] {
            p.position[axis] = o.min[axis];
            p.velocity[axis] = p.velocity[axis].abs() * 0.15;
        } else if p.position[axis] > o.max[axis] {
            p.position[axis] = o.max[axis];
            p.velocity[axis] = -p.velocity[axis].abs() * 0.15;
        }
    }
    let Some(floor) = floor else {
        return;
    };
    let x = ((p.position[0] - floor.origin[0]) / floor.cell).floor() as usize;
    let z = ((p.position[2] - floor.origin[1]) / floor.cell).floor() as usize;
    if x < floor.width && z < floor.depth {
        let bed = floor.heights[z * floor.width + x] + o.smoothing_radius * 0.25;
        if p.position[1] < bed {
            p.position[1] = bed;
            p.velocity[1] = p.velocity[1].abs() * 0.15;
        }
    }
}

pub(super) fn poly6(r2: f32, h: f32) -> f32 {
    let q = h * h - r2;
    if q <= 0.0 {
        0.0
    } else {
        315.0 / (64.0 * std::f32::consts::PI * h.powi(9)) * q.powi(3)
    }
}

pub(super) fn grad_spiky(r: [f32; 3], h: f32) -> [f32; 3] {
    let length = dot(r, r).sqrt();
    if length <= 1e-6 || length >= h {
        return [0.0; 3];
    }
    let factor = -45.0 / (std::f32::consts::PI * h.powi(6)) * (h - length).powi(2) / length;
    mul(r, factor)
}

pub(super) fn distance_sq(a: [f32; 3], b: [f32; 3]) -> f32 {
    dot(sub(a, b), sub(a, b))
}

pub(super) fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub(super) fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(super) fn mul(a: [f32; 3], b: f32) -> [f32; 3] {
    [a[0] * b, a[1] * b, a[2] * b]
}

pub(super) fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(super) fn bound_magnitude(vector: [f32; 3], maximum: f32) -> [f32; 3] {
    let length_sq = dot(vector, vector);
    if !length_sq.is_finite() || !maximum.is_finite() || maximum <= 0.0 {
        [0.0; 3]
    } else if length_sq > maximum * maximum {
        mul(vector, maximum / length_sq.sqrt())
    } else {
        vector
    }
}
