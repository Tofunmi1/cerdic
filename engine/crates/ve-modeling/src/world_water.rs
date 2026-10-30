//! Shared open water sheets and their per-vertex current, depth and shore foam.
use ve_graph::Node;
use ve_kernel::Mesh;

use crate::appearance::{textured_surface_default, with_surfaces};
use crate::finish::Finish;
use crate::water_look::WaterLook;
use crate::world_terrain::{Tile, param};
use crate::{Fail, cache::Value, fail, fail_suggest};

/// Borrowed river or lake data, in the tile grid's row-major order.
pub(crate) struct WaterSheet<'a> {
    pub channel: &'a [f32],
    pub surface: &'a [f32],
    pub flow: Option<(&'a [f32], &'a [f32])>,
    pub speed: &'a [f32],
    pub bank: Option<&'a [f32]>,
    /// Lakes keep the one-cell skirt at the surface height too.
    pub level: Option<f32>,
}

/// What the water sheet shows about one cell.
struct Cell {
    height: f32,
    color: [f32; 4],
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "parameters are bounded and the counts are small"
)]
pub(crate) fn sheet(node: &Node, tile: &Tile, data: WaterSheet<'_>) -> Result<Value, Fail> {
    let WaterSheet {
        channel,
        surface,
        flow,
        speed,
        bank,
        level,
    } = data;
    let (iw, id) = tile.inner_cells();
    let grid = &tile.grid;
    let at = |i: usize, j: usize| (j + tile.margin) * grid.width() + i + tile.margin;
    let cells: Vec<Cell> = (0..id)
        .flat_map(|j| (0..iw).map(move |i| (i, j)))
        .map(|(i, j)| {
            let index = at(i, j);
            let depth = (surface[index] - grid.heights()[index]).max(0.0);
            let shore = bank.map_or_else(
                || 1.0 - smoothstep(0.55, 1.0, channel[index]),
                |values| values[index],
            );
            let (flow_x, flow_z) = flow.map_or((0.0, 0.0), |(x, z)| (x[index], z[index]));
            let foam = (shore + speed[index] * 0.12).clamp(0.0, 1.0);
            Cell {
                height: level.unwrap_or(surface[index]),
                color: [flow_x * 0.5 + 0.5, flow_z * 0.5 + 0.5, depth / 6.0, foam],
            }
        })
        .collect();
    // A quad is water when any corner is in the channel; the vertices are shared and unused ones dropped.
    let mut remap = vec![u32::MAX; iw * id];
    let (mut positions, mut colors, mut indices) = (Vec::new(), Vec::new(), Vec::new());
    for j in 0..id - 1 {
        for i in 0..iw - 1 {
            let corners = [(i, j), (i, j + 1), (i + 1, j + 1), (i + 1, j)];
            if !corners.iter().any(|(ci, cj)| channel[at(*ci, *cj)] > 0.5) {
                continue;
            }
            let ids = corners.map(|(ci, cj)| {
                let slot = cj * iw + ci;
                if remap[slot] == u32::MAX {
                    remap[slot] = u32::try_from(positions.len()).unwrap_or(0);
                    positions.push([
                        grid.world_x(ci + tile.margin),
                        cells[slot].height,
                        grid.world_z(cj + tile.margin),
                    ]);
                    colors.push(cells[slot].color);
                }
                remap[slot]
            });
            indices.extend_from_slice(&[ids[0], ids[1], ids[2], ids[0], ids[2], ids[3]]);
        }
    }
    if indices.is_empty() {
        return Err(fail_suggest(
            "river.empty",
            "the river does not cross this tile, so there is no water to draw",
            "check the river.carve path, or leave this tile without water",
        ));
    }
    let (sim, sim_resolution, sim_damping, sim_depth, rain) = crate::water_look::simulation(node);
    let look = WaterLook {
        volumetric: node.op.family().as_str() == "water.fill",
        sim,
        sim_resolution,
        sim_damping,
        sim_depth,
        rain,
        shallow: triple(node, "shallow")?,
        deep: triple(node, "deep")?,
        clarity: param(node, "clarity")? as f32,
        flow: param(node, "flow")? as f32,
        ripple: param(node, "ripple")? as f32,
        foam: param(node, "foam")? as f32,
        direction: param(node, "direction")?.to_degrees() as f32,
        seed: crate::profile_ops::whole(node, "seed")
            .map_err(|error| crate::operations::kernel_fail(node, &error))?,
    };
    let mut value = Value::solid(Mesh { positions, indices }, 0.0);
    value.colors = colors;
    let opacity = param(node, "opacity")? as f32;
    let styled = with_surfaces(&value, |existing| {
        let mut material = textured_surface_default(existing);
        material.opacity = opacity;
        material.roughness = 0.08;
        material.finish = Some(Finish {
            water: Some(look),
            ..Finish::plain()
        });
        material
    });
    Ok(styled)
}

fn triple(node: &Node, name: &str) -> Result<[f32; 3], Fail> {
    let Some(ve_graph::Value::List(items)) = node.params.get(name) else {
        return Err(fail(
            "param.type",
            format!("{} {name} must be three numbers", node.op.family()),
        ));
    };
    let values: Vec<f64> = items.iter().filter_map(ve_graph::Value::as_f64).collect();
    #[expect(
        clippy::cast_possible_truncation,
        reason = "colour channels are 0 to 1"
    )]
    match values.as_slice() {
        [r, g, b] => Ok([*r as f32, *g as f32, *b as f32]),
        _ => Err(fail(
            "param.type",
            format!("{} {name} must be three numbers", node.op.family()),
        )),
    }
}

fn smoothstep(start: f32, end: f32, value: f32) -> f32 {
    let t = ((value - start) / (end - start).max(f32::EPSILON)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
