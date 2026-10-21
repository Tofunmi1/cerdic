//! Tile-aware heightfield grids, world-space generators and mask layers (ADR 0056).
//!
//! Everything that depends on position uses world coordinates, so two tiles that share an edge agree on it and a tile built
//! alone equals the same area cut from a larger grid. No graph, Bevy or platform dependency.
mod error;
mod generate;
mod grid;
mod mask;
mod mesh;
#[cfg(test)]
mod review_tests;

pub use error::TerrainError;
pub use generate::{
    Noise, StampShape, carve_path, domain_warp, edge_falloff, generate, smooth, stamp, terrace,
};
pub use grid::{HeightGrid, MAX_SIDE};
pub use mask::{Combine, Mask, combine, compute_mask, remap};
pub use mesh::{MeshOut, pack_splat, to_height_texture, to_mesh};
