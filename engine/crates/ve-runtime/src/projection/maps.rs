//! Baked texture maps of the scene being staged: intake per material region, and layer assignment at commit.
use super::{STATE, staging};
use std::collections::{HashMap, HashSet};

/// Baked maps of one material region, in the order albedo, normal, metallic-roughness, emissive.
pub(super) type Maps = [Option<Vec<u8>>; 4];

/// Stages the baked maps (albedo, normal, metallic-roughness, emissive; an empty slice is absent) of one
/// material region. Each must be a square RGBA8 image of one of the renderer's map sizes (1024, 512 or 256 texels); a wrong
/// size refuses all of them.
pub(crate) fn maps(text: &str, node: &str, region: u32, maps: [&[u8]; 4]) -> bool {
    if ve_graph::NodeId::parse(node).is_err()
        || maps
            .iter()
            .any(|m| !m.is_empty() && ve_render::tier_of_len(m.len()).is_none())
    {
        return false;
    }
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(candidate) = state.candidate.as_mut() else {
            return false;
        };
        // Identical maps (tiled parts that share a program) share one layer, so only distinct pixels count.
        let mut distinct: HashSet<&[u8]> = candidate
            .maps
            .values()
            .flatten()
            .flatten()
            .map(Vec::as_slice)
            .collect();
        distinct.extend(maps.iter().copied().filter(|m| !m.is_empty()));
        // The renderer's budget is bytes on the GPU (mip chains included), so a scene of small maps holds many more.
        let cost: usize = distinct.iter().filter_map(|m| ve_render::map_cost(m.len())).sum();
        if serde_json::to_string(&candidate.token).ok().as_deref() != Some(text)
            || !staging::current(&candidate.token)
            || candidate.finished
            || cost > ve_render::BUDGET_BYTES
        {
            return false;
        }
        candidate.maps.insert(
            (node.to_owned(), region),
            maps.map(|m| (!m.is_empty()).then(|| m.to_vec())),
        );
        true
    })
}

/// Replaces the shared texture array with this scene's maps and returns the layers each region was given
/// (-1 for a map that is absent or did not fit).
pub(super) fn assign_layers(
    textures: &mut ve_render::MaterialTextures,
    maps: HashMap<(String, u32), Maps>,
) -> HashMap<(String, u32), [i32; 4]> {
    textures.clear();
    let mut layers = HashMap::new();
    let mut shared: HashMap<Vec<u8>, i32> = HashMap::new();
    for (key, maps) in maps {
        let mut assigned = [-1_i32; 4];
        for (slot, pixels) in maps.into_iter().enumerate() {
            let Some(pixels) = pixels else { continue };
            if let Some(&layer) = shared.get(&pixels) {
                assigned[slot] = layer;
            } else if let Some(layer) = textures.add(pixels.clone()) {
                shared.insert(pixels, layer);
                assigned[slot] = layer;
            }
        }
        layers.insert(key, assigned);
    }
    layers
}
