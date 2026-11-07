//! The walls above the shop floor: one shaft for a low building, one setback for a tall one, and with `detail` two setbacks on a tower (a stepped, New York
//! crown). Returns the outline the roof is capped over.
use super::{Building, BuildingParams, Rng, inset};

pub(super) fn shaft(
    building: &mut Building,
    outline: &[[f32; 2]],
    params: &BuildingParams,
    shop: f32,
    rng: &mut Rng,
) -> Vec<[f32; 2]> {
    let (height, floor) = (params.height, params.floor);
    let tiers = height >= 50.0;
    let main_top = if tiers {
        let floors = ((height * 0.62) / floor).floor().max(1.0);
        (floors * floor).min(height - floor * 3.0).max(shop + floor)
    } else {
        height
    };
    building.walls.walls(outline, shop, main_top, floor);
    if !tiers {
        return outline.to_vec();
    }
    let half_width = outline
        .iter()
        .flatten()
        .fold(f32::MAX, |m, c| m.min(c.abs()));
    let setback = (1.5 + 1.5 * rng.next()).min((half_width * 0.25).max(1.5));
    let upper = inset(outline, setback);
    building.roof.ring(outline, &upper, main_top);
    if height < 90.0 || params.detail < 0.3 {
        building.walls.walls(&upper, main_top, height, floor);
        return upper;
    }
    // A tower with detail steps back a second time, halfway between the first ledge and the top.
    let mid =
        (((main_top + (height - main_top) * 0.5) / floor).floor() * floor).max(main_top + floor);
    building.walls.walls(&upper, main_top, mid, floor);
    let top = inset(
        &upper,
        (2.0 + 2.0 * rng.next()).min((half_width * 0.25).max(1.5)),
    );
    building.roof.ring(&upper, &top, mid);
    building.walls.walls(&top, mid, height, floor);
    top
}
