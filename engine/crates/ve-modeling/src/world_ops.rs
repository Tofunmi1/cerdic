//! `terrain.mask` and `terrain.erode`: shape and read the height grid behind a `terrain.tile` (ADR 0056, algorithms in `ve_world`).
//!
//! Both work on the tile's margin as well as the tile, and erosion regenerates the ground around the tile from its source, so
//! neighbouring tiles eroded or masked with the same settings still meet without a seam.
use std::rc::Rc;

use ve_graph::{Node, Value as Param};
use ve_kernel::ExecutionContext;
use ve_world::erosion::{
    Constraint, Grid, Halo, Hydraulic, StreamPower, Thermal, coarse_world, halo_for, refine_tile,
};
use ve_world::terrain::{Combine, HeightGrid, Mask, combine, compute_mask, remap};

use crate::world_terrain::{Tile, map_error, param, tile_value};
use crate::{Fail, cache::Value, fail, fail_suggest};

pub(crate) fn run(
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Option<Result<Value, Fail>> {
    match node.op.family().as_str() {
        "water.fill" => Some(crate::world_lake::fill(node, inputs, context)),
        "terrain.mask" => Some(mask(node, inputs, context)),
        "terrain.biomes" => Some(biomes(node, inputs)),
        "terrain.erode" => Some(erode(node, inputs, context)),
        _ => None,
    }
}

pub(crate) fn input_tile<'a>(
    node: &Node,
    inputs: &'a [Rc<Value>],
) -> Result<(&'a Value, &'a Tile), Fail> {
    let family = node.op.family();
    let value = inputs.first().ok_or_else(|| {
        fail(
            "graph.missing_input",
            format!("{} needs a terrain.tile", family.as_str()),
        )
    })?;
    let tile = value.grid.as_deref().ok_or_else(|| {
        fail_suggest(
            "terrain.needs_tile",
            format!(
                "{} needs the output of terrain.tile (or another terrain node) as input",
                family.as_str()
            ),
            "connect a terrain.tile node, before solid.material",
        )
    })?;
    Ok((value, tile))
}

pub(crate) fn text<'a>(node: &'a Node, name: &str) -> &'a str {
    match node.params.get(name) {
        Some(Param::Str(value)) => value.as_str(),
        _ => "",
    }
}

fn flag(node: &Node, name: &str) -> bool {
    matches!(node.params.get(name), Some(Param::Bool(true)))
}

pub(crate) fn with_tile(input: &Value, tile: Tile) -> Result<Value, Fail> {
    let mut value = tile_value(tile)?;
    value.material.clone_from(&input.material);
    Ok(value)
}

#[expect(clippy::cast_possible_truncation, reason = "mask bounds are 0 to 1")]
fn mask(node: &Node, inputs: &[Rc<Value>], context: &ExecutionContext) -> Result<Value, Fail> {
    let (input, tile) = input_tile(node, inputs)?;
    let kind = text(node, "mask");
    let mut values = match kind {
        // Relative to the world's tallest ground, not to this window's own range, so every tile agrees.
        "height" => {
            let top = tile.source.height.max(f32::EPSILON);
            tile.grid
                .heights()
                .iter()
                .map(|h| (h / top).clamp(0.0, 1.0))
                .collect()
        }
        other => {
            let which = match other {
                "slope" => Mask::Slope,
                "curvature" => Mask::Curvature,
                "aspect" => Mask::Aspect,
                "occlusion" => Mask::Occlusion,
                _ => return Err(fail("param.invalid", format!("unknown mask {other:?}"))),
            };
            compute_mask(&tile.grid, which, &|| context.is_cancelled()).map_err(map_error)?
        }
    };
    remap(
        &mut values,
        param(node, "from")? as f32,
        param(node, "to")? as f32,
        flag(node, "smooth"),
    );
    if flag(node, "invert") {
        let ones = vec![1.0; values.len()];
        values = combine(&ones, &values, Combine::Subtract);
    }
    let name = if text(node, "name").is_empty() {
        kind
    } else {
        text(node, "name")
    };
    let mut next = tile.clone();
    next.grid.set_layer(name, values).map_err(map_error)?;
    let mut value = input.clone();
    value.grid = Some(Rc::new(next));
    Ok(value)
}

/// `terrain.biomes`: one layer per biome (`prefix` + name), plus `moisture` and `slope_degrees`, computed by `ve_world::biome`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "parameters are small bounded numbers"
)]
fn biomes(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let (input, tile) = input_tile(node, inputs)?;
    let rules = ve_world::biome::parse(text(node, "rules")).map_err(|message| {
        fail_suggest(
            "biome.rules",
            format!("terrain.biomes rules: {message}"),
            "write `name: height low..high, slope low..high, wet low..high; ...`",
        )
    })?;
    let weights = ve_world::biome::weigh(
        &tile.grid,
        &rules,
        param(node, "blend")? as f32,
        param(node, "wet_scale")? as f32,
        param(node, "seed")? as u32,
    );
    let prefix = text(node, "prefix");
    let mut next = tile.clone();
    for (biome, layer) in rules.iter().zip(weights.biomes) {
        next.grid
            .set_layer(&format!("{prefix}{}", biome.name), layer)
            .map_err(map_error)?;
    }
    next.grid
        .set_layer("moisture", weights.wet)
        .map_err(map_error)?;
    next.grid
        .set_layer("slope_degrees", weights.slope)
        .map_err(map_error)?;
    let mut value = input.clone();
    value.grid = Some(Rc::new(next));
    Ok(value)
}

struct Erosion {
    stream: StreamPower,
    thermal: Thermal,
    hydraulic: Hydraulic,
    coarse_cell: f32,
    world_size: f32,
    pull: f32,
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "parameters are small bounded numbers"
)]
fn erosion(node: &Node) -> Result<Erosion, Fail> {
    let talus = param(node, "talus")? as f32;
    let thermal_iterations = if talus > 0.0 {
        param(node, "thermal_iterations")? as u32
    } else {
        0
    };
    let rain = param(node, "rain")? as f32;
    let hydraulic_iterations = if rain > 0.0 {
        param(node, "hydraulic_iterations")? as u32
    } else {
        0
    };
    let river = param(node, "river_erosion")? as f32;
    Ok(Erosion {
        stream: StreamPower {
            k: river * 0.0008,
            m: 0.5,
            uplift: 0.0,
            dt: 1.0,
            iterations: if river > 0.0 {
                param(node, "river_iterations")? as u32
            } else {
                0
            },
        },
        thermal: Thermal {
            talus_deg: if talus > 0.0 { talus } else { 35.0 },
            rate: 0.5,
            iterations: thermal_iterations,
        },
        hydraulic: Hydraulic {
            rain: rain.max(0.0),
            evaporation: 0.3,
            capacity: 2.0,
            dissolve: 0.4,
            deposit: 0.4,
            iterations: hydraulic_iterations,
        },
        coarse_cell: param(node, "coarse_cell")? as f32,
        world_size: param(node, "world_size")? as f32,
        pull: param(node, "pull")? as f32,
    })
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "cell counts are bounded and checked"
)]
fn erode(node: &Node, inputs: &[Rc<Value>], context: &ExecutionContext) -> Result<Value, Fail> {
    let (input, tile) = input_tile(node, inputs)?;
    let settings = erosion(node)?;
    let cancel = || context.is_cancelled();
    let cell = tile.grid.cell();
    // The tile already has a margin; the erosion window adds the reach of its own passes beyond that, then the halo is cut off again.
    let halo = halo_for(&settings.thermal, &settings.hydraulic).cells;
    let (width, depth) = (tile.grid.width() + 2 * halo, tile.grid.depth() + 2 * halo);
    if width > ve_world::terrain::MAX_SIDE || depth > ve_world::terrain::MAX_SIDE {
        return Err(fail_suggest(
            "terrain.too_large",
            format!(
                "eroding this tile needs {width} by {depth} cells including its halo, over the limit of {}",
                ve_world::terrain::MAX_SIDE
            ),
            "lower thermal_iterations or hydraulic_iterations, or use smaller tiles",
        ));
    }
    let origin = [
        tile.grid.world_x(0) - halo as f32 * cell,
        tile.grid.world_z(0) - halo as f32 * cell,
    ];
    let mut window = tile.source.window(origin, cell, width, depth, context)?;

    // The coarse world: the same land at low resolution, eroded once as a whole. Every tile computes the same one.
    let side = ((settings.world_size / settings.coarse_cell).round() as usize + 1).clamp(8, 513);
    let coarse_origin = [-settings.world_size * 0.5, -settings.world_size * 0.5];
    let mut coarse =
        tile.source
            .window(coarse_origin, settings.coarse_cell, side, side, context)?;
    if settings.stream.iterations > 0 {
        let mut grid = Grid {
            heights: coarse.heights_mut(),
            width: side,
            depth: side,
            cell: settings.coarse_cell,
        };
        coarse_world(&mut grid, &settings.stream, &cancel).map_err(erosion_error)?;
    }
    let constraint = Constraint {
        coarse_heights: coarse.heights(),
        coarse_width: side,
        coarse_depth: side,
        coarse_cell: settings.coarse_cell,
        origin: coarse_origin,
        strength: settings.pull,
    };
    let (w, d) = (window.width(), window.depth());
    let hydrology = {
        let mut grid = Grid {
            heights: window.heights_mut(),
            width: w,
            depth: d,
            cell,
        };
        refine_tile(
            &mut grid,
            origin,
            &Halo { cells: halo },
            &constraint,
            &settings.thermal,
            &settings.hydraulic,
            &cancel,
        )
        .map_err(erosion_error)?
    };
    window
        .set_layer("water", hydrology.water)
        .map_err(map_error)?;
    window
        .set_layer("sediment", hydrology.sediment)
        .map_err(map_error)?;

    // Keep the tile and its margin, cut the halo off. Masks computed before erosion no longer describe the ground and are dropped.
    let mut fresh = HeightGrid::new(
        tile.grid.width(),
        tile.grid.depth(),
        cell,
        tile.grid.origin(),
    )
    .map_err(map_error)?;
    fresh.paste(&window).map_err(map_error)?;
    let next = Tile {
        grid: fresh,
        margin: tile.margin,
        source: tile.source,
    };
    with_tile(input, next)
}

fn erosion_error(error: ve_world::erosion::ErosionError) -> Fail {
    match error {
        ve_world::erosion::ErosionError::Cancelled => {
            fail("eval.cancelled", "terrain erosion cancelled")
        }
        other => fail("terrain.failed", other.to_string()),
    }
}
