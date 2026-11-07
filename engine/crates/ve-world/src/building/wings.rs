//! Splitting a big rectangular lot into wings so a block is not a row of boxes: an L (a street wing and a narrower back wing) or a U (a street wing with two back
//! wings round a courtyard). Every wing is a convex rectangle the building generator already accepts, so each is built as its own building.
use super::Rng;

/// One wing of a lot.
#[derive(Clone, Debug, PartialEq)]
pub struct Wing {
    /// The wing's outline in (x, z) metres, counter-clockwise.
    pub footprint: Vec<[f32; 2]>,
    /// The footprint edge that faces the street side of the lot (shops, awnings and fire escapes go there).
    pub front: usize,
    /// How tall the wing is next to the street wing's height: 1 for the street wing, less behind it.
    pub height_factor: f32,
}

/// Both sides of a lot need at least this much for a courtyard to make sense.
const MIN_SIDE: f32 = 16.0;

/// The wings of a lot.
///
/// A lot that is not a rectangle, or is under 16 m on a side, stays one wing, as does every lot at `share` 0; at `share` 1 every big rectangular lot splits.
/// `front` is the lot's street edge. The same lot, front and seed always give the same wings.
#[must_use]
pub fn wings(lot: &[[f32; 2]], front: usize, seed: u64, share: f32) -> Vec<Wing> {
    let whole = || {
        vec![Wing {
            footprint: lot.to_vec(),
            front,
            height_factor: 1.0,
        }]
    };
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x1234_5678_9ABC_DEF1);
    if lot.len() != 4 || front >= 4 || rng.next() >= share {
        return whole();
    }
    let a = lot[front];
    let (b, d) = (lot[(front + 1) % 4], lot[(front + 3) % 4]);
    let (along, depth) = ([b[0] - a[0], b[1] - a[1]], [d[0] - a[0], d[1] - a[1]]);
    let (width, deep) = (length(along), length(depth));
    let rectangle = (along[0] * depth[0] + along[1] * depth[1]).abs() < 0.02 * width * deep;
    if !rectangle || width < MIN_SIDE || deep < MIN_SIDE {
        return whole();
    }
    let (t, m) = (
        [along[0] / width, along[1] / width],
        [depth[0] / deep, depth[1] / deep],
    );
    let rect = |s0: f32, s1: f32, d0: f32, d1: f32, height_factor: f32| {
        let at = |s: f32, k: f32| [a[0] + t[0] * s + m[0] * k, a[1] + t[1] * s + m[1] * k];
        let corners = [at(s0, d0), at(s1, d0), at(s1, d1), at(s0, d1)];
        let (footprint, front) = if signed_area(&corners) >= 0.0 {
            (corners.to_vec(), 0)
        } else {
            (corners.iter().rev().copied().collect(), 2)
        };
        Wing {
            footprint,
            front,
            height_factor,
        }
    };
    let street = 0.4 + 0.15 * rng.next();
    let back = 0.6 + 0.2 * rng.next();
    let (street_depth, down) = (deep * street, back);
    let mut found = vec![rect(0.0, width, 0.0, street_depth, 1.0)];
    if rng.next() < 0.5 {
        // an L: the back wing fills one side
        let left = rng.next() < 0.5;
        let (s0, s1) = if left {
            (0.0, width * 0.45)
        } else {
            (width * 0.55, width)
        };
        found.push(rect(s0, s1, street_depth, deep, down));
    } else {
        // a U: two back wings, a courtyard between them
        found.push(rect(0.0, width * 0.3, street_depth, deep, down));
        found.push(rect(width * 0.7, width, street_depth, deep, down));
    }
    found
}

fn length(v: [f32; 2]) -> f32 {
    (v[0] * v[0] + v[1] * v[1]).sqrt()
}

fn signed_area(polygon: &[[f32; 2]]) -> f32 {
    (0..polygon.len())
        .map(|i| {
            let (a, b) = (polygon[i], polygon[(i + 1) % polygon.len()]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f32>()
        * 0.5
}
