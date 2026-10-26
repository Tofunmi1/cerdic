//! Connected lakes and their wet banks on a borrowed ground grid.
use super::{GroundRef, RiverError, RiverOut};
use crate::num::{cell_f32, floor_index, offset};

/// A still lake's surface and the wet ground around its shore, in world metres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LakeProfile {
    /// Water surface height in world metres.
    pub level: f32,
    /// Width of the wet bank beyond the first shore cell, in metres; zero disables it.
    pub shore_width: f32,
}

/// Fills the four-connected cells below `level` starting under world `(x, z)` `seed`.
/// Dry cells keep their ground height; all current vectors and speeds are zero.
/// A two-pass eight-neighbour chamfer measures bank distance, with a smooth fade
/// from the first adjacent cell to zero one shore width farther out (within one
/// cell of the requested distance). Lakes may reach the grid border.
///
/// # Errors
/// `BadShape` or `BadParameter` for invalid ground, non-finite `level`, or negative
/// or non-finite `shore_width`. An invalid/outside seed or a seed whose cell is
/// not below `level` returns `BadParameter` named `seed`, including when the level
/// is below all ground. `Cancelled` when the callback requests cancellation.
pub fn fill_basin(
    ground: &GroundRef<'_>,
    seed: [f32; 2],
    profile: &LakeProfile,
    cancel: &dyn Fn() -> bool,
) -> Result<RiverOut, RiverError> {
    ground.check()?;
    for (name, valid) in [
        ("level", profile.level.is_finite()),
        (
            "shore_width",
            profile.shore_width.is_finite() && profile.shore_width >= 0.0,
        ),
    ] {
        if !valid {
            return Err(RiverError::BadParameter {
                name,
                why: "level must be finite; shore_width must be finite and non-negative".into(),
            });
        }
    }
    poll(0, cancel)?;
    let start = seed_cell(ground, seed, profile.level)?;
    let count = ground.heights.len();
    let mut out = RiverOut {
        water_height: ground.heights.to_vec(),
        flow_dir: vec![[0.0; 2]; count],
        flow_speed: vec![0.0; count],
        bank_mask: vec![0.0; count],
        channel_mask: vec![0.0; count],
    };
    let mut queue = vec![start];
    out.channel_mask[start] = 1.0;
    let mut head = 0;
    while head < queue.len() {
        poll(head, cancel)?;
        let index = queue[head];
        head += 1;
        out.water_height[index] = profile.level;
        let (x, z) = (index % ground.width, index / ground.width);
        for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            if let (Some(nx), Some(nz)) = (offset(x, dx, ground.width), offset(z, dz, ground.depth))
            {
                let next = nz * ground.width + nx;
                if out.channel_mask[next] < 0.5 && ground.heights[next] < profile.level {
                    out.channel_mask[next] = 1.0;
                    queue.push(next);
                }
            }
        }
    }
    if profile.shore_width > 0.0 {
        banks(ground, profile.shore_width, &mut out, cancel)?;
    }
    Ok(out)
}

fn seed_cell(ground: &GroundRef<'_>, seed: [f32; 2], level: f32) -> Result<usize, RiverError> {
    let x = (seed[0] - ground.origin[0]) / ground.cell;
    let z = (seed[1] - ground.origin[1]) / ground.cell;
    if !seed.iter().all(|n| n.is_finite())
        || !x.is_finite()
        || !z.is_finite()
        || x < 0.0
        || z < 0.0
        || x > cell_f32(ground.width - 1)
        || z > cell_f32(ground.depth - 1)
    {
        return Err(RiverError::BadParameter {
            name: "seed",
            why: "the seed must be finite and inside the ground grid".into(),
        });
    }
    let index = floor_index(z) * ground.width + floor_index(x);
    if !ground.heights[index].is_finite() || ground.heights[index] >= level {
        return Err(RiverError::BadParameter {
            name: "seed",
            why: "the seed cell is not in a basin below level".into(),
        });
    }
    Ok(index)
}

fn poll(index: usize, cancel: &dyn Fn() -> bool) -> Result<(), RiverError> {
    if index.is_multiple_of(2048) && cancel() {
        return Err(RiverError::Cancelled);
    }
    Ok(())
}

fn banks(
    ground: &GroundRef<'_>,
    shore: f32,
    out: &mut RiverOut,
    cancel: &dyn Fn() -> bool,
) -> Result<(), RiverError> {
    let mut distance = vec![f32::INFINITY; out.channel_mask.len()];
    for (i, value) in distance.iter_mut().enumerate() {
        poll(i, cancel)?;
        if out.channel_mask[i] > 0.5 {
            *value = 0.0;
        }
    }
    for reverse in [false, true] {
        for step in 0..distance.len() {
            poll(step, cancel)?;
            let i = if reverse {
                distance.len() - 1 - step
            } else {
                step
            };
            let (x, z) = (i % ground.width, i / ground.width);
            let sign = if reverse { -1 } else { 1 };
            for (dx, dz, weight) in [
                (-1, 0, 1.0),
                (-1, -1, std::f32::consts::SQRT_2),
                (0, -1, 1.0),
                (1, -1, std::f32::consts::SQRT_2),
            ] {
                if let (Some(nx), Some(nz)) = (
                    offset(x, dx * sign, ground.width),
                    offset(z, dz * sign, ground.depth),
                ) {
                    distance[i] = distance[i].min(distance[nz * ground.width + nx] + weight);
                }
            }
        }
    }
    for (i, distance) in distance.iter().enumerate() {
        poll(i, cancel)?;
        if out.channel_mask[i] < 0.5 {
            let t = ((distance - 1.0) * ground.cell / shore).clamp(0.0, 1.0);
            out.bank_mask[i] = 1.0 - t * t * (3.0 - 2.0 * t);
        }
    }
    Ok(())
}
