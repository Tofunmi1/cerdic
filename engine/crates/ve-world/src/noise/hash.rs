//! Integer hashing and gradient vectors shared by every noise basis. No floating-point hashing, so every platform agrees.

/// A 64-bit finaliser (the splitmix64 mixer): every input bit changes about half the output bits.
pub(super) fn mix(mut h: u64) -> u64 {
    h = (h ^ (h >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^ (h >> 31)
}

/// The hash of a lattice cell: the first `dims` entries of `cell` under `seed`.
pub(super) fn cell_hash(seed: u32, cell: [i64; 4], dims: usize) -> u64 {
    let mut h = mix(u64::from(seed) ^ 0x9E37_79B9_7F4A_7C15);
    for coordinate in cell.iter().take(dims) {
        h = mix(h ^ coordinate.cast_unsigned());
    }
    h
}

/// A value in `[0, 1)` from the top 53 bits of a hash.
pub(super) fn unit(h: u64) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "53 hash bits fit an f64 mantissa exactly"
    )]
    let value = (h >> 11) as f64;
    value / 9_007_199_254_740_992.0
}

const SQRT_HALF: f64 = std::f64::consts::FRAC_1_SQRT_2;
/// `1 / sqrt(3)`.
const THIRD_ROOT: f64 = 0.577_350_269_189_625_8;

/// A unit gradient for a cell hash: 8 directions in 2D, the 12 edge directions in 3D, 32 in 4D.
/// Only the first `dims` entries are meaningful.
pub(super) fn gradient(h: u64, dims: usize) -> [f64; 4] {
    let mut g = [0.0; 4];
    let sign = |bit: u64| if bit == 0 { 1.0 } else { -1.0 };
    match dims {
        2 => {
            let k = h >> 40 & 7;
            if k < 4 {
                g[usize::from(k & 1 == 1)] = sign(k >> 1 & 1);
            } else {
                g[0] = sign(k & 1) * SQRT_HALF;
                g[1] = sign(k >> 1 & 1) * SQRT_HALF;
            }
        }
        3 => {
            let k = (h >> 40) % 12;
            let zero = usize::try_from(k / 4).unwrap_or(0);
            let others = [(zero + 1) % 3, (zero + 2) % 3];
            g[others[0]] = sign(k & 1) * SQRT_HALF;
            g[others[1]] = sign(k >> 1 & 1) * SQRT_HALF;
        }
        _ => {
            let k = h >> 40 & 31;
            let zero = usize::try_from(k / 8).unwrap_or(0);
            let mut bit = 0;
            for (axis, slot) in g.iter_mut().enumerate() {
                if axis != zero {
                    *slot = sign(k >> bit & 1) * THIRD_ROOT;
                    bit += 1;
                }
            }
        }
    }
    g
}
