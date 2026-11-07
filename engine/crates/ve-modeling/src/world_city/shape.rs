//! The buildings of one lot for `city.buildings`: the lot as one box or as wings, each with a flat or a mansard roof, picked by a hash of the lot so the same
//! district always gets the same shapes.
use ve_world::building::{BuildingParams, Roof, inset, wings};
use ve_world::city::Lot;

/// What the node asks for besides the per-lot basics.
pub(super) struct Shape {
    pub seed: u64,
    pub floor: f32,
    pub clutter: f32,
    pub detail: f32,
    pub scale: f32,
    pub height_min: f32,
    pub height_max: f32,
    pub mansard: f32,
    pub courtyards: f32,
}

/// Which of `styles` facade styles lot `index` belongs to.
pub(super) fn style_of(index: usize, seed: u64, styles: u64) -> u64 {
    let mut z = (index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ seed.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 29)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z >> 33) % styles
}

/// Whether lot `index` falls inside `share`, a different draw for each `tag`.
fn picked(index: usize, seed: u64, tag: u64, share: f32) -> bool {
    let mut z = (index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ seed.wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ tag;
    z = (z ^ (z >> 30)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    #[expect(clippy::cast_precision_loss, reason = "24 bits of a hash")]
    let draw = (z >> 40) as f32 / 16_777_216.0;
    draw < share
}

/// The buildings of `lot`: one per wing.
pub(super) fn lot_buildings(lot: &Lot, index: usize, shape: &Shape) -> Vec<BuildingParams> {
    let base = shape
        .seed
        .wrapping_mul(1_000_003)
        .wrapping_add(index as u64);
    let roof = if picked(index, shape.seed, 0x51, shape.mansard) {
        Roof::Mansard
    } else {
        Roof::Flat
    };
    let height = (lot.height * shape.scale).clamp(shape.height_min, shape.height_max);
    wings(
        &inset(&lot.footprint, 0.08),
        lot.front,
        base,
        shape.courtyards,
    )
    .into_iter()
    .enumerate()
    .map(|(k, wing)| BuildingParams {
        footprint: wing.footprint,
        height: (height * wing.height_factor).clamp(3.0, shape.height_max),
        front: wing.front,
        seed: base.wrapping_add(k as u64 * 7_919),
        floor: shape.floor,
        clutter: shape.clutter,
        detail: shape.detail,
        roof,
    })
    .collect()
}
