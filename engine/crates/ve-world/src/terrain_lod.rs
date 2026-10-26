//! Balanced, camera-dependent quadtree selection and streaming changes for terrain.
//!
//! Selection is independent of rendering and preserves world-grid coordinates at every level.
mod select;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

pub use select::select_tiles;

/// Distances and grid scale for terrain selection, in world metres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LodOptions {
    /// Edge length of a finest (level 0) tile; must be finite and positive.
    pub tile_size: f64,
    /// Number of levels, including level 0; must be nonzero.
    pub levels: usize,
    /// Level 0 selection range; must be finite and positive.
    pub base_range: f64,
    /// Range multiplier per level; must be finite and positive.
    pub range_ratio: f64,
    /// Last fraction of each range used for morphing, in `0..=1`; zero disables morphing.
    pub morph: f64,
}

/// One selected leaf of the terrain quadtree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelectedTile {
    /// Level 0 is finest; each successive level doubles the edge length.
    pub level: usize,
    /// Signed x coordinate in this level's grid.
    pub x: i64,
    /// Signed z coordinate in this level's grid.
    pub z: i64,
    /// Minimum world x and z, equal to `(x, z) * size`.
    pub min: [f64; 2],
    /// World edge length, equal to `tile_size * 2^level`.
    pub size: f64,
    /// Blend toward the coarser parent, clamped to `0..=1`.
    pub morph: f64,
}

/// Changes in tile residency, keyed by `(level, x, z)` rather than morph or position.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TileDiff {
    /// New tiles to request.
    pub load: Vec<SelectedTile>,
    /// Old tiles no longer requested.
    pub unload: Vec<SelectedTile>,
}

/// Lists additions and removals in `(level, z, x)` order.
///
/// A morph-only change does not change residency. Duplicate keys produce at most one entry;
/// the first occurrence supplies that entry's data.
#[must_use]
pub fn diff_tiles(previous: &[SelectedTile], next: &[SelectedTile]) -> TileDiff {
    let keyed = |tiles: &[SelectedTile]| {
        let mut map = BTreeMap::new();
        for tile in tiles {
            map.entry((tile.level, tile.z, tile.x)).or_insert(*tile);
        }
        map
    };
    let previous = keyed(previous);
    let next = keyed(next);
    TileDiff {
        load: next
            .iter()
            .filter(|(key, _)| !previous.contains_key(key))
            .map(|(_, tile)| *tile)
            .collect(),
        unload: previous
            .iter()
            .filter(|(key, _)| !next.contains_key(key))
            .map(|(_, tile)| *tile)
            .collect(),
    }
}
