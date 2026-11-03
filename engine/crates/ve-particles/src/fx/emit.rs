//! Birth: the random stream, the birth shape and the birth direction.
use super::Shape;

/// A splitmix64 stream: the same seed always gives the same particles.
#[derive(Clone, Debug)]
pub(super) struct Rng(u64);

impl Rng {
    pub(super) fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// A number in `[0, 1)`.
    pub(super) fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        f32::from_bits(0x3F80_0000 | u32::try_from(z >> 41).unwrap_or_default()) - 1.0
    }

    pub(super) fn between(&mut self, range: [f32; 2]) -> f32 {
        range[0] + (range[1] - range[0]) * self.next()
    }

    fn signed(&mut self) -> f32 {
        self.next() * 2.0 - 1.0
    }
}

fn length(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn normalise(v: [f32; 3]) -> [f32; 3] {
    let l = length(v);
    if l > 0.0 {
        [v[0] / l, v[1] / l, v[2] / l]
    } else {
        [0.0, 1.0, 0.0]
    }
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// A birth offset from the shape's centre.
pub(super) fn offset(shape: &Shape, rng: &mut Rng) -> [f32; 3] {
    match *shape {
        Shape::Point => [0.0; 3],
        Shape::Sphere { radius } => {
            let direction = normalise([rng.signed(), rng.signed(), rng.signed()]);
            let reach = radius * rng.next();
            direction.map(|c| c * reach)
        }
        Shape::Box { half_extents } => [
            rng.signed() * half_extents[0],
            rng.signed() * half_extents[1],
            rng.signed() * half_extents[2],
        ],
        Shape::Disc { radius } => {
            // Rejection sampling keeps the disc uniform without trigonometry.
            loop {
                let (x, z) = (rng.signed(), rng.signed());
                if x * x + z * z <= 1.0 {
                    return [x * radius, 0.0, z * radius];
                }
            }
        }
        Shape::Line { length } => [0.0, rng.signed() * length * 0.5, 0.0],
    }
}

/// A unit birth direction: `direction` spread by about `cone` radians.
pub(super) fn direction(base: [f32; 3], cone: f32, rng: &mut Rng) -> [f32; 3] {
    let base = normalise(base);
    if cone <= 0.0 {
        return base;
    }
    let reference = if base[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
    let tangent = normalise(cross(base, reference));
    let bitangent = normalise(cross(base, tangent));
    let (jx, jy) = (rng.signed() * cone, rng.signed() * cone);
    normalise([
        base[0] + tangent[0] * jx + bitangent[0] * jy,
        base[1] + tangent[1] * jx + bitangent[1] * jy,
        base[2] + tangent[2] * jx + bitangent[2] * jy,
    ])
}
