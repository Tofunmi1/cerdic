//! Grouping the copies of an instance set into spatially compact chunks, so a whole chunk can be kept or culled with one test and
//! drawn with one call from a run of the set's retained instance buffer.

use std::ops::Range;

use bevy_math::Vec3;

/// A run of consecutive copies (in the set's sorted order) and a sphere that holds all of them.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Chunk {
    pub(crate) centre: Vec3,
    pub(crate) radius: f32,
    pub(crate) range: Range<u32>,
}

/// How many copies make a chunk for a set of `count`: about 48 chunks a set, between 32 and 512 copies each, so a big set is cut
/// finely enough to cull well and a small one is not drowned in draw calls.
pub(crate) fn chunk_size_for(count: usize) -> usize {
    (count / 48).clamp(32, 512)
}

/// Spreads the low 10 bits of `v` so there are two zero bits between each.
fn spread(v: u32) -> u32 {
    let mut x = v & 0x3ff;
    x = (x | (x << 16)) & 0x0300_00ff;
    x = (x | (x << 8)) & 0x0300_f00f;
    x = (x | (x << 4)) & 0x030c_30c3;
    (x | (x << 2)) & 0x0924_9249
}

/// The Morton (Z-order) code of a point inside `low..high`: nearby points get nearby codes.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "each coordinate is clamped to 0..=1 and scaled to 0..=1023 before it becomes an integer"
)]
fn morton(point: Vec3, low: Vec3, size: Vec3) -> u32 {
    let unit = |value: f32, low: f32, size: f32| -> u32 {
        if size <= 0.0 {
            return 0;
        }
        (((value - low) / size).clamp(0.0, 1.0) * 1023.0) as u32
    };
    spread(unit(point.x, low.x, size.x))
        | (spread(unit(point.y, low.y, size.y)) << 1)
        | (spread(unit(point.z, low.z, size.z)) << 2)
}

/// Sorts copies (given as a centre and radius each) along a Z-order curve and cuts them into chunks of `chunk_size`. Returns the
/// order (position `i` of the sorted set holds copy `order[i]`) and the chunks over that order. Deterministic: ties keep input order.
pub(crate) fn build(spheres: &[(Vec3, f32)], chunk_size: usize) -> (Vec<u32>, Vec<Chunk>) {
    let chunk_size = chunk_size.max(1);
    if spheres.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let (low, high) = spheres.iter().fold(
        (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
        |(low, high), (centre, _)| (low.min(*centre), high.max(*centre)),
    );
    let size = high - low;
    let mut keyed: Vec<(u32, u32)> = spheres
        .iter()
        .enumerate()
        .map(|(index, (centre, _))| {
            (
                morton(*centre, low, size),
                u32::try_from(index).unwrap_or(u32::MAX),
            )
        })
        .collect();
    keyed.sort_unstable();
    let order: Vec<u32> = keyed.into_iter().map(|(_, index)| index).collect();
    let chunks = order
        .chunks(chunk_size)
        .enumerate()
        .map(|(n, members)| {
            let start = n * chunk_size;
            let (low, high) = members.iter().fold(
                (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
                |(low, high), &i| {
                    let centre = spheres[i as usize].0;
                    (low.min(centre), high.max(centre))
                },
            );
            let centre = (low + high) * 0.5;
            let radius = members
                .iter()
                .map(|&i| {
                    let (c, r) = spheres[i as usize];
                    c.distance(centre) + r
                })
                .fold(0.0_f32, f32::max);
            Chunk {
                centre,
                radius,
                range: u32::try_from(start).unwrap_or(u32::MAX)
                    ..u32::try_from(start + members.len()).unwrap_or(u32::MAX),
            }
        })
        .collect();
    (order, chunks)
}

#[cfg(test)]
#[path = "set_chunks_tests.rs"]
mod tests;
