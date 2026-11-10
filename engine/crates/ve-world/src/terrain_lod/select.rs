//! Validated grid arithmetic, range selection and explicit neighbour balancing.
use std::collections::{BTreeMap, BTreeSet};

use super::{LodOptions, SelectedTile};

// Key order also defines the public deterministic output order.
type Key = (usize, i64, i64); // level, z, x

struct Selection {
    world_min: [f64; 2],
    world_max: [f64; 2],
    camera: [f64; 3],
    heights: [f64; 2],
    morph: f64,
    scales: Vec<(f64, f64)>, // size, range at each level
}

/// Covers the world with range-selected leaves, then splits coarse edge neighbours until
/// adjacent leaves differ by at most one level. Output is ordered by level, then z, then x.
///
/// Distance is measured to the tile's 3D box, including `height_range`. Only tiles with a
/// positive-area intersection with the world are retained. At unaligned world boundaries,
/// full grid squares overhang the boundary: their intersections cover the world exactly once.
/// The tile containing the camera's x/z position has zero morph regardless of altitude.
///
/// Returns an empty list for non-finite inputs, empty/reversed world bounds, reversed heights,
/// invalid options, overflowing/underflowing level scales, or coordinates outside the exact
/// integer lattice of f64 (absolute finest-grid boundary coordinates over `2^52`).
#[must_use]
pub fn select_tiles(
    world_min: [f64; 2],
    world_max: [f64; 2],
    camera: [f64; 3],
    options: &LodOptions,
    height_range: [f64; 2],
) -> Vec<SelectedTile> {
    let Some(selection) = Selection::new(world_min, world_max, camera, options, height_range)
    else {
        return Vec::new();
    };
    let top = options.levels - 1;
    let size = selection.scales[top].0;
    let start = world_min.map(|v| boundary_index(v, size, false));
    let end = world_max.map(|v| boundary_index(v, size, true));
    // Also reject root bounds that cannot be represented as finite world coordinates.
    if start
        .into_iter()
        .chain(end)
        .any(|v| !(coordinate(v) * size).is_finite())
    {
        return Vec::new();
    }
    let mut pending = Vec::new();
    for z in start[1]..end[1] {
        for x in start[0]..end[0] {
            pending.push(selection.tile((top, z, x)));
        }
    }
    let mut leaves = BTreeMap::new();
    while let Some(tile) = pending.pop() {
        if tile.level > 0 && selection.distance(&tile) < selection.scales[tile.level].1 {
            pending.extend(selection.children(&tile));
        } else {
            leaves.insert((tile.level, tile.z, tile.x), tile);
        }
    }
    selection.balance(&mut leaves);
    leaves.into_values().collect()
}

impl Selection {
    fn new(
        world_min: [f64; 2],
        world_max: [f64; 2],
        camera: [f64; 3],
        options: &LodOptions,
        heights: [f64; 2],
    ) -> Option<Self> {
        if world_min
            .into_iter()
            .chain(world_max)
            .chain(camera)
            .chain(heights)
            .chain([
                options.tile_size,
                options.base_range,
                options.range_ratio,
                options.morph,
            ])
            .any(|v| !v.is_finite())
            || (0..2).any(|i| world_min[i] >= world_max[i])
            || heights[0] > heights[1]
            || options.levels == 0
            || options.tile_size <= 0.0
            || options.base_range <= 0.0
            || options.range_ratio <= 0.0
            || !(0.0..=1.0).contains(&options.morph)
        {
            return None;
        }
        if world_min
            .into_iter()
            .chain(world_max)
            .any(|v| (v / options.tile_size).abs().ceil() > 4_503_599_627_370_496.0)
        {
            return None;
        }
        let mut scales = Vec::new();
        let (mut size, mut range) = (options.tile_size, options.base_range);
        for _ in 0..options.levels {
            if !size.is_finite() || !range.is_finite() || range <= 0.0 {
                return None;
            }
            scales.push((size, range));
            size *= 2.0;
            range *= options.range_ratio;
        }
        Some(Self {
            world_min,
            world_max,
            camera,
            heights,
            morph: options.morph,
            scales,
        })
    }

    fn tile(&self, (level, z, x): Key) -> SelectedTile {
        let (size, range) = self.scales[level];
        let mut tile = SelectedTile {
            level,
            x,
            z,
            min: [coordinate(x) * size, coordinate(z) * size],
            size,
            morph: 0.0,
        };
        let contains_camera = tile.min[0] <= self.camera[0]
            && self.camera[0] < tile.min[0] + size
            && tile.min[1] <= self.camera[2]
            && self.camera[2] < tile.min[1] + size;
        if self.morph > 0.0 && !contains_camera {
            // CDLOD linear range morph: normalise before subtracting to avoid underflow
            // when morph * range is very small. A zero-width morph band is disabled above.
            let fraction = self.distance(&tile) / range;
            tile.morph = if fraction >= 1.0 {
                1.0
            } else {
                ((fraction - (1.0 - self.morph)) / self.morph).clamp(0.0, 1.0)
            };
        }
        tile
    }

    fn distance(&self, tile: &SelectedTile) -> f64 {
        // RedGPU landscapeCull.wgsl uses 3D Euclidean camera distance for its ranges.
        // Adapted to the nearest point of the box instead of its centre, per CDLOD.
        // Copyright (c) 2019~ Redcamel, MIT.
        let gap = |p: f64, min: f64, max: f64| (min - p).max(p - max).max(0.0);
        let dx = gap(self.camera[0], tile.min[0], tile.min[0] + tile.size);
        let dy = gap(self.camera[1], self.heights[0], self.heights[1]);
        let dz = gap(self.camera[2], tile.min[1], tile.min[1] + tile.size);
        dx.hypot(dy).hypot(dz)
    }

    fn children(&self, tile: &SelectedTile) -> Vec<SelectedTile> {
        let mut children = Vec::new();
        for dz in 0..2 {
            for dx in 0..2 {
                // Coordinates are bounded at the finest level by validation, so doubling
                // a parent overlapping the world cannot overflow i64.
                let child = self.tile((tile.level - 1, tile.z * 2 + dz, tile.x * 2 + dx));
                if (0..2).all(|i| {
                    child.min[i] < self.world_max[i]
                        && child.min[i] + child.size > self.world_min[i]
                }) {
                    children.push(child);
                }
            }
        }
        children
    }

    fn balance(&self, leaves: &mut BTreeMap<Key, SelectedTile>) {
        loop {
            let mut split = BTreeSet::new();
            for &(level, z, x) in leaves.keys() {
                for (mut nz, mut nx) in [(z, x - 1), (z, x + 1), (z - 1, x), (z + 1, x)] {
                    // Walk the adjacent cell's ancestors. Euclidean division preserves the
                    // same grid for negative coordinates. Finer neighbours do this walk too.
                    for neighbour_level in level..self.scales.len() {
                        let key = (neighbour_level, nz, nx);
                        if leaves.contains_key(&key) {
                            if neighbour_level > level + 1 {
                                split.insert(key);
                            }
                            break;
                        }
                        nz = nz.div_euclid(2);
                        nx = nx.div_euclid(2);
                    }
                }
            }
            if split.is_empty() {
                break;
            }
            for key in split {
                if let Some(tile) = leaves.remove(&key) {
                    for child in self.children(&tile) {
                        leaves.insert((child.level, child.z, child.x), child);
                    }
                }
            }
        }
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "validated integral f64 coordinates fit i64"
)]
fn boundary_index(value: f64, size: f64, upper: bool) -> i64 {
    let quotient = value / size;
    // Preserve which side of zero a boundary occupies even if division underflows.
    if quotient == 0.0 {
        return if upper {
            i64::from(value > 0.0)
        } else {
            -i64::from(value < 0.0)
        };
    }
    if upper {
        quotient.ceil() as i64
    } else {
        quotient.floor() as i64
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "validated coordinates fit the exact f64 integer lattice"
)]
fn coordinate(value: i64) -> f64 {
    value as f64
}
