//! Baked texture maps of the scene being staged: intake per material region, and layer assignment at commit.
use super::{STATE, staging};
use std::collections::HashMap;

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
        // Identical maps (tiled parts that share a program) share one layer, so only distinct pixels count. The renderer's budget is
        // bytes on the GPU (mip chains included), so a scene of small maps holds many more. Each map is digested once, here.
        let keys = ve_render::MapCosts::keys(&maps);
        let cost = candidate.map_costs.cost_after(&keys);
        if serde_json::to_string(&candidate.token).ok().as_deref() != Some(text)
            || !staging::current(&candidate.token)
            || candidate.finished
            || cost > ve_render::BUDGET_BYTES
        {
            return false;
        }
        candidate.map_costs.add(&keys);
        candidate.maps.insert(
            (node.to_owned(), region),
            maps.map(|m| (!m.is_empty()).then(|| m.to_vec())),
        );
        true
    })
}

type LayerKey = (String, u32);

thread_local! {
    /// The layers the committed scene's maps were given, by part and region.
    static LAYER_INDEX: std::cell::RefCell<HashMap<LayerKey, [i32; 4]>> = std::cell::RefCell::new(HashMap::new());
    /// Live pictures waiting for the next frame: (part, region, slot) and the new RGBA8 pixels.
    static LIVE_PENDING: std::cell::RefCell<Vec<((String, u32, usize), Vec<u8>)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Most live pictures one frame may carry (each part's maps count once; later pictures for the same map replace earlier ones).
const MAX_LIVE_PENDING: usize = 64;

/// Remembers which layers the committed scene's maps live in, so live pictures can find theirs.
pub(super) fn remember_layers(layers: &HashMap<LayerKey, [i32; 4]>) {
    LAYER_INDEX.with(|index| index.borrow_mut().clone_from(layers));
    LIVE_PENDING.with(|pending| pending.borrow_mut().clear());
}

/// Queues a new picture (square RGBA8 of a renderer map size, the same size the map already has) for map `slot` (0 albedo, 1 normal, 2 metallic-roughness,
/// 3 emissive) of the part made by `node`, in `region`. `false` when the scene has no such map or the picture is not a map size.
pub(crate) fn live_update(node: &str, region: u32, slot: u32, pixels: &[u8]) -> bool {
    let Ok(slot) = usize::try_from(slot) else {
        return false;
    };
    if slot > 3 || ve_graph::NodeId::parse(node).is_err() || ve_render::tier_of_len(pixels.len()).is_none() {
        return false;
    }
    let known = LAYER_INDEX.with(|index| index.borrow().get(&(node.to_owned(), region)).is_some_and(|layers| layers[slot] >= 0));
    if !known {
        return false;
    }
    LIVE_PENDING.with(|pending| {
        let mut pending = pending.borrow_mut();
        let key = (node.to_owned(), region, slot);
        if let Some(entry) = pending.iter_mut().find(|(k, _)| *k == key) {
            entry.1 = pixels.to_vec();
            return true;
        }
        if pending.len() >= MAX_LIVE_PENDING {
            return false;
        }
        pending.push((key, pixels.to_vec()));
        true
    })
}

/// Puts the queued live pictures into the renderer's maps (each only if its map is the same size).
pub(crate) fn apply_live(textures: &mut ve_render::MaterialTextures) {
    let queued: Vec<_> = LIVE_PENDING.with(|pending| std::mem::take(&mut *pending.borrow_mut()));
    for ((node, region, slot), pixels) in queued {
        let id = LAYER_INDEX.with(|index| index.borrow().get(&(node, region)).map(|layers| layers[slot]));
        if let Some(id) = id.filter(|id| *id >= 0) {
            let _ = textures.set_layer(id, &pixels);
        }
    }
}

/// Replaces the shared texture array with this scene's maps and returns the layers each region was given
/// (-1 for a map that is absent or did not fit).
pub(super) fn assign_layers(
    textures: &mut ve_render::MaterialTextures,
    maps: HashMap<(String, u32), Maps>,
) -> HashMap<(String, u32), [i32; 4]> {
    textures.clear();
    let mut layers = HashMap::new();
    // Layers already given, by digest; equal digests are confirmed by comparing the pixels the renderer holds.
    let mut shared: HashMap<ve_render::MapKey, Vec<i32>> = HashMap::new();
    for (key, maps) in maps {
        let mut assigned = [-1_i32; 4];
        for (slot, pixels) in maps.into_iter().enumerate() {
            let Some(pixels) = pixels else { continue };
            let digest = (pixels.len(), ve_render::map_digest(&pixels));
            let same = shared.get(&digest).and_then(|layers| {
                layers.iter().copied().find(|&layer| {
                    textures
                        .layer(layer)
                        .is_some_and(|(_, held)| held == pixels)
                })
            });
            if let Some(layer) = same {
                assigned[slot] = layer;
            } else if let Some(layer) = textures.add(pixels) {
                shared.entry(digest).or_default().push(layer);
                assigned[slot] = layer;
            }
        }
        layers.insert(key, assigned);
    }
    layers
}
