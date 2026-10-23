//! Integer-hashed, deterministic 3D noises.
use crate::num::lattice_index;

fn hash(x: i32, y: i32, z: i32, seed: i32) -> u32 {
    let mut value = x.cast_unsigned().wrapping_mul(0x8DA6_B343)
        ^ y.cast_unsigned().wrapping_mul(0xD816_3841)
        ^ z.cast_unsigned().wrapping_mul(0xCB1A_B31F)
        ^ seed.cast_unsigned().wrapping_mul(0x9E37_79B9);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846C_A68B);
    value ^ (value >> 16)
}

fn grad(hash: u32, x: f32, y: f32, z: f32) -> f32 {
    let pair = usize::try_from(hash % 12).unwrap_or(0);
    match pair {
        0 => x + y,
        1 => -x + y,
        2 => x - y,
        3 => -x - y,
        4 => x + z,
        5 => -x + z,
        6 => x - z,
        7 => -x - z,
        8 => y + z,
        9 => -y + z,
        10 => y - z,
        _ => -y - z,
    }
}

fn fade(value: f32) -> f32 {
    value * value * value * (value * (value * 6.0 - 15.0) + 10.0)
}
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + t * (b - a)
}

pub(super) fn perlin(p: [f32; 3], seed: i32) -> f32 {
    if p.iter().any(|v| !v.is_finite()) {
        return 0.0;
    }
    let cell = [
        lattice_index(p[0]),
        lattice_index(p[1]),
        lattice_index(p[2]),
    ];
    let frac = [
        p[0] - crate::num::lattice_f32(cell[0]),
        p[1] - crate::num::lattice_f32(cell[1]),
        p[2] - crate::num::lattice_f32(cell[2]),
    ];
    let mut values = [[[0.0; 2]; 2]; 2];
    for (z, plane) in values.iter_mut().enumerate() {
        for (y, row) in plane.iter_mut().enumerate() {
            for (x, out) in row.iter_mut().enumerate() {
                let dx = frac[0] - bit_float(x);
                let dy = frac[1] - bit_float(y);
                let dz = frac[2] - bit_float(z);
                *out = grad(
                    hash(
                        cell[0] + bit_index(x),
                        cell[1] + bit_index(y),
                        cell[2] + bit_index(z),
                        seed,
                    ),
                    dx,
                    dy,
                    dz,
                );
            }
        }
    }
    let u = fade(frac[0]);
    let v = fade(frac[1]);
    let w = fade(frac[2]);
    let z0y0 = lerp(values[0][0][0], values[0][0][1], u);
    let z0y1 = lerp(values[0][1][0], values[0][1][1], u);
    let z1y0 = lerp(values[1][0][0], values[1][0][1], u);
    let z1y1 = lerp(values[1][1][0], values[1][1][1], u);
    let low = lerp(z0y0, z0y1, v);
    let high = lerp(z1y0, z1y1, v);
    (lerp(low, high, w) * 1.0).clamp(-1.0, 1.0)
}

pub(super) fn fbm(p: [f32; 3], octaves: i32, seed: i32, ridged: bool) -> f32 {
    let mut amplitude = 1.0;
    let mut total = 0.0;
    let mut norm = 0.0;
    let mut point = p;
    for octave in 0_u8..8 {
        if i32::from(octave) >= octaves.clamp(1, 8) {
            break;
        }
        let shift = f32::from(octave) * 37.0;
        let value = perlin(
            [
                point[0] + shift,
                point[1] + shift * 0.73,
                point[2] - shift * 0.41,
            ],
            seed.wrapping_add(i32::from(octave).wrapping_mul(1013)),
        );
        let value = if ridged {
            let ridge = 1.0 - value.abs();
            ridge * ridge
        } else {
            value
        };
        total += value * amplitude;
        norm += amplitude;
        amplitude *= 0.5;
        point = [point[0] * 2.0, point[1] * 2.0, point[2] * 2.0];
    }
    let result = total / norm;
    if ridged {
        result.clamp(0.0, 1.0)
    } else {
        result.clamp(-1.0, 1.0)
    }
}

fn bit_index(index: usize) -> i32 {
    i32::from(index != 0)
}
fn bit_float(index: usize) -> f32 {
    if index == 0 { 0.0 } else { 1.0 }
}
fn unit(value: u32) -> f32 {
    f32::from(u16::try_from(value >> 16).unwrap_or(0)) / 65_536.0
}

pub(super) fn voronoi(position: [f32; 3], seed: i32) -> f32 {
    if position.iter().any(|v| !v.is_finite()) {
        return 0.0;
    }
    let cell = [
        lattice_index(position[0]),
        lattice_index(position[1]),
        lattice_index(position[2]),
    ];
    let mut nearest = f32::MAX;
    for dz in -1..=1 {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let x = cell[0] + dx;
                let y = cell[1] + dy;
                let z = cell[2] + dz;
                let h = hash(x, y, z, seed);
                let point = [
                    crate::num::lattice_f32(x) + unit(h),
                    crate::num::lattice_f32(y) + unit(h.rotate_left(11)),
                    crate::num::lattice_f32(z) + unit(h.rotate_left(22)),
                ];
                let offset = [
                    position[0] - point[0],
                    position[1] - point[1],
                    position[2] - point[2],
                ];
                nearest = nearest.min(
                    (offset[0] * offset[0] + offset[1] * offset[1] + offset[2] * offset[2]).sqrt(),
                );
            }
        }
    }
    if nearest.is_finite() { nearest } else { 0.0 }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "noise seeds are floored and saturate to the hash integer range"
)]
pub(super) fn seed(value: f32) -> i32 {
    if value.is_finite() {
        value.floor() as i32
    } else {
        0
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "the value is clamped to the small integer octave range"
)]
pub(super) fn octaves(value: f32) -> i32 {
    if value.is_finite() {
        value.floor().clamp(1.0, 8.0) as i32
    } else {
        1
    }
}
