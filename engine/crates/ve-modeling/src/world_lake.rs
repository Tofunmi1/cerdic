//! `water.fill`: connected still water in a terrain tile's hollow.
use std::rc::Rc;

use ve_graph::{Node, Value as Param};
use ve_kernel::ExecutionContext;
use ve_world::river::{GroundRef, LakeProfile, RiverError, fill_basin};

use crate::world_ops::input_tile;
use crate::world_terrain::param;
use crate::world_water::{WaterSheet, sheet};
use crate::{Fail, cache::Value, fail};

#[expect(
    clippy::cast_possible_truncation,
    reason = "positions and levels are bounded metres"
)]
pub(crate) fn fill(
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Result<Value, Fail> {
    let (_, tile) = input_tile(node, inputs)?;
    // Use the visible tile, excluding its generated margin, for both seed bounds and flooding.
    let grid = tile.inner()?;
    let at = match node.params.get("at") {
        None => [
            (grid.world_x(0) + grid.world_x(grid.width() - 1)) * 0.5,
            (grid.world_z(0) + grid.world_z(grid.depth() - 1)) * 0.5,
        ],
        Some(Param::List(values)) => match values.as_slice() {
            [x, z] => match (x.as_f64(), z.as_f64()) {
                (Some(x), Some(z)) => [x as f32, z as f32],
                _ => return Err(fail("param.type", "water.fill at must be [x, z]")),
            },
            _ => return Err(fail("param.type", "water.fill at must be [x, z]")),
        },
        Some(_) => return Err(fail("param.type", "water.fill at must be [x, z]")),
    };
    let profile = LakeProfile {
        level: param(node, "level")? as f32,
        shore_width: param(node, "shore")? as f32,
    };
    let out = fill_basin(
        &GroundRef {
            heights: grid.heights(),
            width: grid.width(),
            depth: grid.depth(),
            cell: grid.cell(),
            origin: grid.origin(),
        },
        at,
        &profile,
        &|| context.is_cancelled(),
    )
    .map_err(|error| match error {
        RiverError::Cancelled => fail("eval.cancelled", "water.fill cancelled"),
        RiverError::BadParameter { name: "seed", why } => fail(
            "water.bad_at",
            format!("`at` is not in a basin below `level`: {why}"),
        ),
        other => fail("water.fill_failed", other.to_string()),
    })?;
    let inner_tile = crate::world_terrain::Tile {
        grid,
        margin: 0,
        source: tile.source,
    };
    sheet(
        node,
        &inner_tile,
        WaterSheet {
            channel: &out.channel_mask,
            surface: &out.water_height,
            flow: None,
            speed: &out.flow_speed,
            bank: Some(&out.bank_mask),
            level: Some(profile.level),
        },
    )
}
