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

fn astc_6x6_chain_bytes(side: usize) -> usize {
    let mut total = 0;
    let mut edge = side;
    loop {
        let blocks = edge.div_ceil(6);
        total += blocks * blocks * 16;
        if edge == 1 {
            return total;
        }
        edge /= 2;
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "The texture budget bounds both values below f32's useful range here."
)]
fn budget_fraction(used: usize, budget: usize) -> f32 {
    used as f32 / budget as f32
}

/// What a map of `len` bytes takes on the GPU with its mip chain, or `None` when it is not one of the three sizes.
#[must_use]
pub fn map_cost(len: usize) -> Option<usize> {
    let tier = tier_of_len(len)?;
    Some(chain_texels(TIER_SIZES[tier] as usize) * FORMAT_BYTES)
}

/// A 64-bit digest of a map's pixels and size, fast enough to look at every texel of a 4 MB map once (eight bytes at a time).
/// It tells maps apart for costing only; layers are shared by comparing pixels.
#[must_use]
pub fn map_digest(pixels: &[u8]) -> u64 {
    const K: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut h = (pixels.len() as u64).wrapping_mul(K) ^ 0xCBF2_9CE4_8422_2325;
    let mut words = pixels.chunks_exact(8);
    for word in &mut words {
        let mut bytes = [0_u8; 8];
        bytes.copy_from_slice(word);
        h = (h ^ u64::from_le_bytes(bytes))
            .wrapping_mul(K)
            .rotate_left(29);
    }
    for &byte in words.remainder() {
        h = (h ^ u64::from(byte)).wrapping_mul(K).rotate_left(29);
    }
    h ^ (h >> 32)
}

/// Size and digest of one map.
pub type MapKey = (usize, u64);

/// What the distinct maps of a scene take on the GPU, kept as maps are staged. Each map is digested once when it arrives, so staging
/// a scene of N maps costs N digests, not N squared.
#[derive(Default, Debug)]
pub struct MapCosts {
    seen: std::collections::HashSet<MapKey>,
    bytes: usize,
}

impl MapCosts {
    /// The keys of the non-empty maps, in order.
    #[must_use]
    pub fn keys(maps: &[&[u8]]) -> Vec<MapKey> {
        maps.iter()
            .filter(|m| !m.is_empty())
            .map(|m| (m.len(), map_digest(m)))
            .collect()
    }

    /// Bytes on the GPU of the distinct maps so far.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// What the total would be with these maps added; nothing changes.
    #[must_use]
    pub fn cost_after(&self, keys: &[MapKey]) -> usize {
        let mut fresh = std::collections::HashSet::new();
        let extra: usize = keys
            .iter()
            .filter(|key| !self.seen.contains(key) && fresh.insert(**key))
            .filter_map(|(len, _)| map_cost(*len))
            .sum();
        self.bytes + extra
    }

    /// Adds these maps; one already seen costs nothing.
    pub fn add(&mut self, keys: &[MapKey]) {
        self.bytes = self.cost_after(keys);
        self.seen.extend(keys.iter().copied());
    }
}

/// The maps wanted by the scene, kept in the main world and copied to the render world when they change.
#[derive(Resource, Clone, Default, Debug)]
pub struct MaterialTextures {
    /// RGBA8 pixels of each layer of each tier, row 0 first.
    tiers: [Vec<Vec<u8>>; 3],
    /// Bytes the maps take on the GPU, mip chains included.
    bytes: usize,
    generation: u64,
    /// How many times each map's pixels were replaced in place by [`MaterialTextures::set_layer`] (live textures); absent means never.
    versions: std::collections::HashMap<i32, u64>,
    /// Changes whenever any map is replaced in place.
    live_generation: u64,
}

/// A summary of the scene's distinct texture maps and their estimated GPU storage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextureReport {
    /// Map counts at 1024, 512 and 256 texels per side.
    pub maps: [usize; 3],
    /// RGBA8 GPU bytes by size, including mip chains.
    pub bytes: [usize; 3],
    /// Total RGBA8 GPU bytes, equal to [`MaterialTextures::bytes`].
    pub total_bytes: usize,
    /// Maximum GPU bytes allowed for the scene's maps.
    pub budget_bytes: usize,
    /// The fraction of the map budget used.
    pub used_fraction: f32,
    /// Estimate of GPU bytes with ASTC 6x6 compression, including rounded mip chains.
    pub astc_6x6_estimate_bytes: usize,
}

impl MaterialTextures {
    /// Reports map counts and GPU storage by size, including an ASTC 6x6 estimate.
    #[must_use]
    pub fn report(&self) -> TextureReport {
        let maps = self.tiers.each_ref().map(Vec::len);
        let bytes = std::array::from_fn(|tier| {
            maps[tier] * chain_texels(TIER_SIZES[tier] as usize) * FORMAT_BYTES
        });
        let astc_6x6_estimate_bytes = self
            .tiers
            .iter()
            .zip(TIER_SIZES)
            .map(|(maps, side)| maps.len() * astc_6x6_chain_bytes(side as usize))
            .sum();
        TextureReport {
            maps,
            bytes,
            total_bytes: self.bytes,
            budget_bytes: BUDGET_BYTES,
            used_fraction: budget_fraction(self.bytes, BUDGET_BYTES),
            astc_6x6_estimate_bytes,
        }
    }

    /// Removes every map. Surfaces holding map ids must be replaced too.
    pub fn clear(&mut self) {
        if self.bytes != 0 || self.tiers.iter().any(|t| !t.is_empty()) {
            self.tiers = Default::default();
            self.bytes = 0;
            self.versions.clear();
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

    /// The map with this id (see [`TIER_STRIDE`]): its side in texels and its RGBA8 pixels.
    #[must_use]
    pub fn layer(&self, id: i32) -> Option<(u32, &[u8])> {
        let tier = usize::try_from(id / TIER_STRIDE).ok()?;
        let layer = usize::try_from(id % TIER_STRIDE).ok()?;
        let pixels = self.tiers.get(tier)?.get(layer)?;
        Some((*TIER_SIZES.get(tier)?, pixels))
    }

    /// Replaces the pixels of map `id` with `rgba` of the same size (a live texture drawn again this frame). `false` when there is no such map or the
    /// size differs. Does not change [`generation`](Self::generation), so every other map keeps its image; see [`layer_version`](Self::layer_version).
    pub fn set_layer(&mut self, id: i32, rgba: &[u8]) -> bool {
        let (Ok(tier), Ok(layer)) = (usize::try_from(id / TIER_STRIDE), usize::try_from(id % TIER_STRIDE)) else {
            return false;
        };
        let Some(held) = self.tiers.get_mut(tier).and_then(|t| t.get_mut(layer)) else {
            return false;
        };
        if held.len() != rgba.len() {
            return false;
        }
        held.copy_from_slice(rgba);
        *self.versions.entry(id).or_insert(0) += 1;
        self.live_generation += 1;
        true
    }

    /// How many times map `id` was replaced in place (0 for a map that never was).
    #[must_use]
    pub fn layer_version(&self, id: i32) -> u64 {
        self.versions.get(&id).copied().unwrap_or(0)
    }

    /// Changes whenever any map is replaced in place with [`set_layer`](Self::set_layer).
    #[must_use]
    pub fn live_generation(&self) -> u64 {
        self.live_generation
    }

    /// Changes whenever a map is added or the maps are cleared.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
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
