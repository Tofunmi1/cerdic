//! Warped street grid generation.
use super::{CityParams, Random};

/// Street graph together with its un-subdivided block junction grid.
pub(super) struct StreetGrid {
    /// Street vertices.
    pub(super) nodes: Vec<[f32; 2]>,
    /// Street graph edges.
    pub(super) edges: Vec<(u32, u32)>,
    /// Number of street lines along x.
    pub(super) x_lines: usize,
    /// Number of street lines along z.
    pub(super) z_lines: usize,
    /// Subdivided mesh width.
    pub(super) x_points: usize,
}

impl StreetGrid {
    /// Generate a connected street mesh, subdivided to roughly one third block spacing.
    pub(super) fn new(params: &CityParams) -> Self {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "validated district extents and minimum block spacing bound each axis to 67 cells"
        )]
        let x_lines = (params.size[0] / params.block).ceil() as usize;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "validated district extents and minimum block spacing bound each axis to 67 cells"
        )]
        let z_lines = (params.size[1] / params.depth()).ceil() as usize;
        let x_points = x_lines * 3;
        let z_points = z_lines * 3;
        // Irregularity 0 is a regular grid: evenly spaced streets (above 0 they keep the jitter they always had).
        let regular = params.irregularity == 0.0;
        let x_axis = axis(
            params.size[0],
            x_lines,
            params.block,
            params.seed,
            0x58,
            regular,
        );
        let z_axis = axis(
            params.size[1],
            z_lines,
            params.depth(),
            params.seed,
            0x5a,
            regular,
        );
        let rng = Random::new(params.seed);
        let mut nodes = Vec::with_capacity((x_points + 1) * (z_points + 1));
        for iz in 0..=z_points {
            let z = interpolate_axis(&z_axis, iz, z_points);
            for ix in 0..=x_points {
                let x = interpolate_axis(&x_axis, ix, x_points);
                let (dx, dz) = displacement(params, [x, z], &rng);
                nodes.push([
                    (x + dx).clamp(-params.size[0] * 0.5, params.size[0] * 0.5),
                    (z + dz).clamp(-params.size[1] * 0.5, params.size[1] * 0.5),
                ]);
            }
        }
        let mut edges = Vec::with_capacity(x_points * (z_points + 1) + z_points * (x_points + 1));
        for iz in 0..=z_points {
            for ix in 0..x_points {
                edges.push((node_id(ix, iz, x_points), node_id(ix + 1, iz, x_points)));
            }
        }
        for iz in 0..z_points {
            for ix in 0..=x_points {
                edges.push((node_id(ix, iz, x_points), node_id(ix, iz + 1, x_points)));
            }
        }
        Self {
            nodes,
            edges,
            x_lines,
            z_lines,
            x_points,
        }
    }

    /// Find a street junction by its x and z line indices.
    pub(super) fn junction(&self, x: usize, z: usize) -> [f32; 2] {
        let ix = x * 3;
        let iz = z * 3;
        self.nodes[node_id(ix, iz, self.x_points) as usize]
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "validated street counts do not exceed 67"
)]
fn axis(size: f32, count: usize, block: f32, seed: u64, tag: u64, regular: bool) -> Vec<f32> {
    let half = size * 0.5;
    let spacing = size / count as f32;
    let rng = Random::new(seed);
    (0..=count)
        .map(|i| {
            let base = -half + spacing * i as f32;
            if i == 0 || i == count || regular {
                base
            } else {
                base + rng.signed(i as u64, tag) * block * 0.15
            }
        })
        .collect()
}

#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "interpolation indices are nonnegative and bounded by 201 subdivided points"
)]
fn interpolate_axis(axis: &[f32], point: usize, points: usize) -> f32 {
    let intervals = axis.len() - 1;
    let scaled = point as f32 * intervals as f32 / points as f32;
    let index = (scaled.floor() as usize).min(intervals - 1);
    let fraction = scaled - index as f32;
    axis[index] + (axis[index + 1] - axis[index]) * fraction
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "validated district dimensions bound the subdivided node index below 40402"
)]
fn node_id(x: usize, z: usize, width: usize) -> u32 {
    (z * (width + 1) + x) as u32
}

fn displacement(params: &CityParams, point: [f32; 2], rng: &Random) -> (f32, f32) {
    if params.irregularity == 0.0 {
        return (0.0, 0.0);
    }
    let scale = params.block * 1.7;
    let x = point[0] / scale;
    let z = point[1] / scale;
    let first = [
        value_noise(x, z, rng, 0x11),
        value_noise(x + 13.7, z - 8.3, rng, 0x22),
    ];
    let second = [
        value_noise(x * 2.1, z * 2.1, rng, 0x33),
        value_noise(x * 2.1 - 4.2, z * 2.1 + 6.1, rng, 0x44),
    ];
    let amplitude = params.block * 0.11 * params.irregularity;
    (
        amplitude * (first[0] * 0.75 + second[0] * 0.25),
        amplitude * (first[1] * 0.75 + second[1] * 0.25),
    )
}

#[expect(
    clippy::many_single_char_names,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "small coordinate names match value-noise notation; noise lattice coordinates stay within a few cells"
)]
fn value_noise(x: f32, z: f32, rng: &Random, tag: u64) -> f32 {
    let ix = x.floor() as i32;
    let iz = z.floor() as i32;
    let fx = smooth(x - ix as f32);
    let fz = smooth(z - iz as f32);
    let a = lattice(ix, iz, rng, tag);
    let b = lattice(ix + 1, iz, rng, tag);
    let c = lattice(ix, iz + 1, rng, tag);
    let d = lattice(ix + 1, iz + 1, rng, tag);
    lerp(lerp(a, b, fx), lerp(c, d, fx), fz)
}

fn lattice(x: i32, z: i32, rng: &Random, tag: u64) -> f32 {
    let coordinate = i64::from(x)
        .cast_unsigned()
        .wrapping_mul(0x632b_e59b_d9b4_e019)
        ^ i64::from(z)
            .cast_unsigned()
            .wrapping_mul(0x9e37_79b9_7f4a_7c15);
    rng.signed(coordinate, tag)
}

fn smooth(value: f32) -> f32 {
    value * value * (3.0 - 2.0 * value)
}
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displacement_is_smooth_seeded_and_zero_at_zero_irregularity() {
        let params = CityParams::default();
        let rng = Random::new(params.seed);
        let a = displacement(&params, [12.0, 18.0], &rng);
        assert_eq!(a, displacement(&params, [12.0, 18.0], &rng));
        let b = displacement(&params, [12.1, 18.0], &rng);
        assert!((a.0 - b.0).abs() < 0.2 && (a.1 - b.1).abs() < 0.2);
        let straight = CityParams {
            irregularity: 0.0,
            ..params
        };
        assert_eq!((0.0, 0.0), displacement(&straight, [12.0, 18.0], &rng));
    }
}
