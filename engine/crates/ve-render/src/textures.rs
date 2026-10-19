//! The shared material texture arrays: baked maps (albedo, normal, metallic-roughness, emissive) of textured surfaces.
//!
//! Maps come in three sizes (1024, 512 and 256 texels square), each size in its own texture array, so a leaf or a
//! rough wall does not pay for a hero texture. What the scene may hold is a byte budget (counting each map's mip
//! chain), not a number of layers: thirty-odd large maps, or a hundred and more small ones.
//!
//! A surface refers to its maps by id (see `VeMaterial::map_layers`): `tier * 4096 + layer`, where tier 0 is the
//! 1024 array, so ids of large maps are plain layer numbers. Every draw shares one bind group, so batching still
//! depends only on the mesh. The arrays are rebuilt when maps change.

use bevy_ecs::prelude::*;
use bevy_render::Extract;

pub(crate) use crate::textures_gpu::{TexturesGpu, prepare_textures, textures_layout};

/// Side of the largest maps, in texels.
pub const LAYER_SIZE: u32 = 1024;
/// Sides of the three map sizes, largest first (tier 0, 1 and 2).
pub const TIER_SIZES: [u32; 3] = [1024, 512, 256];
/// What all the maps of a scene may take on the GPU, mip chains included. Thirty-three 1024 maps, 136 of 512 or 546 of 256.
pub const BUDGET_BYTES: usize = 192 * 1024 * 1024;
/// Ids of tier `t` start at `t * TIER_STRIDE`.
pub const TIER_STRIDE: i32 = 4096;
const FORMAT_BYTES: usize = 4;

/// The tier of a map of `len` bytes (RGBA8, square), if it is one of the three sizes.
#[must_use]
pub fn tier_of_len(len: usize) -> Option<usize> {
    TIER_SIZES
        .iter()
        .position(|&side| len == side as usize * side as usize * FORMAT_BYTES)
}

/// Texels in a full mip chain of a `side` square map.
fn chain_texels(side: usize) -> usize {
    let mut total = 0;
    let mut edge = side;
    loop {
        total += edge * edge;
        if edge == 1 {
            return total;
        }
        edge /= 2;
    }
}

/// What a map of `len` bytes takes on the GPU with its mip chain, or `None` when it is not one of the three sizes.
#[must_use]
pub fn map_cost(len: usize) -> Option<usize> {
    let tier = tier_of_len(len)?;
    Some(chain_texels(TIER_SIZES[tier] as usize) * FORMAT_BYTES)
}

/// The maps wanted by the scene, kept in the main world and copied to the render world when they change.
#[derive(Resource, Clone, Default, Debug)]
pub struct MaterialTextures {
    /// RGBA8 pixels of each layer of each tier, row 0 first.
    tiers: [Vec<Vec<u8>>; 3],
    /// Bytes the maps take on the GPU, mip chains included.
    bytes: usize,
    generation: u64,
}

impl MaterialTextures {
    /// Removes every map. Surfaces holding map ids must be replaced too.
    pub fn clear(&mut self) {
        if self.bytes != 0 || self.tiers.iter().any(|t| !t.is_empty()) {
            self.tiers = Default::default();
            self.bytes = 0;
            self.generation += 1;
        }
    }

    /// Adds one map and returns its id, or `None` when the budget is spent or the data is not a square RGBA8 image of
    /// one of the [`TIER_SIZES`].
    pub fn add(&mut self, rgba: Vec<u8>) -> Option<i32> {
        let tier = tier_of_len(rgba.len())?;
        let cost = map_cost(rgba.len())?;
        if self.bytes + cost > BUDGET_BYTES {
            return None;
        }
        let layer = i32::try_from(self.tiers[tier].len()).ok()?;
        self.tiers[tier].push(rgba);
        self.bytes += cost;
        self.generation += 1;
        i32::try_from(tier)
            .ok()?
            .checked_mul(TIER_STRIDE)?
            .checked_add(layer)
    }

    /// How many maps are in use.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tiers.iter().map(Vec::len).sum()
    }

    /// Whether no map is in use.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Bytes the maps take on the GPU, mip chains included.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.bytes
    }
}

/// The render world's copy of [`MaterialTextures`].
#[derive(Resource, Default)]
pub(crate) struct ExtractedTextures {
    pub(crate) tiers: [Vec<Vec<u8>>; 3],
    generation: u64,
    pub(crate) changed: bool,
}

/// Copies the maps across only when they changed.
pub(crate) fn extract_textures(
    mut out: ResMut<ExtractedTextures>,
    source: Extract<Res<MaterialTextures>>,
) {
    out.changed = false;
    let filled = source.tiers.iter().any(|t| !t.is_empty());
    let empty = out.tiers.iter().all(Vec::is_empty);
    if out.generation != source.generation || (empty && filled) {
        out.tiers.clone_from(&source.tiers);
        out.generation = source.generation;
        out.changed = true;
    }
}

#[cfg(test)]
#[path = "textures_tests.rs"]
mod tests;
