//! `river.carve` and `river.water`: a river cut into a terrain tile and its water surface (ADR 0059, geometry by `ve_world::river`).
//!
//! `river.carve` stores what the water needs as layers of the tile (`channel`, `bank`, `water_height`, `flow_x`, `flow_z`, `flow_speed`);
//! `river.water` turns them into a sheet that carries flow, depth and foam per vertex, drawn by the water shader.
use std::rc::Rc;

use ve_graph::{Node, Value as Param};
use ve_kernel::ExecutionContext;
use ve_world::river::{
    Ground, Path, RiverError, RiverProfile, carve_river, carve_river_descending,
};

use crate::world_ops::{input_tile, text, with_tile};
use crate::world_terrain::{Tile, param};
use crate::{Fail, cache::Value, fail, fail_suggest};

pub(crate) fn run(
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Option<Result<Value, Fail>> {
    match node.op.family().as_str() {
        "river.carve" => Some(carve(node, inputs, context)),
        "river.water" => Some(water(node, inputs)),
        _ => None,
    }
}

fn river_error(error: RiverError) -> Fail {
    match error {
        RiverError::Cancelled => fail("eval.cancelled", "river carving cancelled"),
        RiverError::BadPath { why } => fail_suggest(
            "river.bad_path",
            format!("the river path is unusable: {why}"),
            "give at least two different x,z points that cross the tile, for example path=\"10,64; 100,70\"",
        ),
        other => fail("river.failed", other.to_string()),
    }
}

/// `x,z; x,z; ...` into a path; a malformed pair names itself.
fn parse_path(node: &Node) -> Result<Path, Fail> {
    let mut points = Vec::new();
    for pair in text(node, "path")
        .split(';')
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        let mut numbers = pair.split(',').map(|n| n.trim().parse::<f32>());
        match (numbers.next(), numbers.next(), numbers.next()) {
            (Some(Ok(x)), Some(Ok(z)), None) if x.is_finite() && z.is_finite() => {
                points.push([x, z])
            }
            _ => {
                return Err(fail_suggest(
                    "river.bad_path",
                    format!("{pair:?} is not an x,z pair"),
                    "write the path as `x,z; x,z; ...` in world metres, for example path=\"10,64; 100,70\"",
                ));
            }
        }
    }
    Ok(Path { points })
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "parameters are bounded metres and degrees"
)]
fn carve(node: &Node, inputs: &[Rc<Value>], context: &ExecutionContext) -> Result<Value, Fail> {
    let (input, tile) = input_tile(node, inputs)?;
    let path = parse_path(node)?;
    let profile = RiverProfile {
        width: param(node, "width")? as f32,
        depth: param(node, "depth")? as f32,
        bank_slope_deg: param(node, "bank_slope")? as f32,
        bank_width: param(node, "bank_width")? as f32,
        meander_flatten: param(node, "flatten")? as f32,
        water_level_offset: param(node, "water_offset")? as f32,
    };
    let descending = match node.params.get("level") {
        None => false,
        Some(Param::Str(level)) if level == "local" => false,
        Some(Param::Str(level)) if level == "descending" => true,
        Some(Param::Str(level)) => {
            return Err(fail(
                "river.bad_level",
                format!("unknown river level {level:?}; choose local or descending"),
            ));
        }
        Some(_) => {
            return Err(fail(
                "param.type",
                "river level must be local or descending",
            ));
        }
    };
    let mut grid = tile.grid.clone();
    let (width, depth, cell, origin) = (grid.width(), grid.depth(), grid.cell(), grid.origin());
    let out = {
        let mut ground = Ground {
            heights: grid.heights_mut(),
            width,
            depth,
            cell,
            origin,
        };
        let carve = if descending {
            carve_river_descending(&mut ground, &path, &profile, &|| context.is_cancelled())
        } else {
            carve_river(&mut ground, &path, &profile, &|| context.is_cancelled())
        };
        carve.map_err(river_error)?
    };
    let flow_x: Vec<f32> = out.flow_dir.iter().map(|d| d[0]).collect();
    let flow_z: Vec<f32> = out.flow_dir.iter().map(|d| d[1]).collect();
    for (name, values) in [
        ("channel", out.channel_mask),
        ("bank", out.bank_mask),
        ("water_height", out.water_height),
        ("flow_x", flow_x),
        ("flow_z", flow_z),
        ("flow_speed", out.flow_speed),
    ] {
        grid.set_layer(name, values)
            .map_err(crate::world_terrain::map_error)?;
    }
    with_tile(
        input,
        Tile {
            grid,
            margin: tile.margin,
            source: tile.source,
        },
    )
}

fn water(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let (_, tile) = input_tile(node, inputs)?;
    let layer = |name: &str| {
        tile.grid.layer(name).ok_or_else(|| {
            fail_suggest(
                "river.needs_carve",
                format!("the tile has no {name:?} layer"),
                "connect the output of river.carve (before solid.material) to river.water",
            )
        })
    };
    crate::world_water::sheet(
        node,
        tile,
        crate::world_water::WaterSheet {
            channel: layer("channel")?,
            surface: layer("water_height")?,
            flow: Some((layer("flow_x")?, layer("flow_z")?)),
            speed: layer("flow_speed")?,
            bank: None,
            level: None,
        },
    )
}
