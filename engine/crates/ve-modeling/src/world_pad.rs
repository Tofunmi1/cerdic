//! `terrain.flatten` and `terrain.place`: a level pad in a tile's ground, and copies standing on the ground at listed positions.
//!
//! Flatten changes the whole grid, margin included, from world coordinates only, so a pad that crosses a tile edge still meets itself
//! on the neighbouring tile when both tiles get the same node.
use std::rc::Rc;

use glam::{DMat4, DQuat, DVec3};
use ve_graph::{Node, Value as Param};

use crate::instances::{InstanceSet, MAX_INSTANCES};
use crate::world_ops::{input_tile, text, with_tile};
use crate::world_terrain::{Tile, param};
use crate::{Fail, cache::Value, fail, fail_suggest};

pub(crate) fn run(node: &Node, inputs: &[Rc<Value>]) -> Option<Result<Value, Fail>> {
    match node.op.family().as_str() {
        "terrain.flatten" => Some(flatten(node, inputs)),
        "terrain.place" => Some(place(node, inputs)),
        _ => None,
    }
}

/// A list of numbers of exactly `width` entries from a param, or none when the param is not a list of lists.
fn rows(node: &Node, name: &str, width: usize) -> Result<Vec<Vec<f64>>, Fail> {
    let Some(Param::List(entries)) = node.params.get(name) else {
        return Ok(Vec::new());
    };
    entries
        .iter()
        .map(|entry| match entry {
            Param::List(numbers) => {
                let values: Vec<f64> = numbers.iter().filter_map(Param::as_f64).collect();
                if values.len() == width {
                    Ok(values)
                } else {
                    Err(fail(
                        "param.type",
                        format!("every entry of {name} needs {width} numbers"),
                    ))
                }
            }
            _ => Err(fail(
                "param.type",
                format!("{name} must be a list of [..] entries"),
            )),
        })
        .collect()
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "positions and radii are bounded metres"
)]
fn flatten(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let (input, tile) = input_tile(node, inputs)?;
    let centre = rows(node, "at", 2).or_else(|_| {
        // `at` is a single [x, z] pair, not a list of pairs.
        match node.params.get("at") {
            Some(Param::List(items)) if items.len() == 2 => Ok(vec![
                items
                    .iter()
                    .map(|n| n.as_f64().unwrap_or(0.0))
                    .collect::<Vec<f64>>(),
            ]),
            _ => Err(fail("param.type", "terrain.flatten at must be [x, z]")),
        }
    })?;
    let [x, z] = match centre.first().map(Vec::as_slice) {
        Some([x, z]) => [*x as f32, *z as f32],
        _ => return Err(fail("param.type", "terrain.flatten at must be [x, z]")),
    };
    let (radius, blend) = (param(node, "radius")? as f32, param(node, "blend")? as f32);
    let level = if text(node, "to") == "height" {
        param(node, "height")? as f32
    } else {
        tile.grid.sample(x, z)
    };
    let mut next = tile.clone();
    let (width, depth) = (next.grid.width(), next.grid.depth());
    let columns: Vec<f32> = (0..width).map(|i| next.grid.world_x(i)).collect();
    let rows_z: Vec<f32> = (0..depth).map(|j| next.grid.world_z(j)).collect();
    let heights = next.grid.heights_mut();
    for (j, wz) in rows_z.iter().enumerate() {
        for (i, wx) in columns.iter().enumerate() {
            let distance = ((wx - x).powi(2) + (wz - z).powi(2)).sqrt();
            let weight = if distance <= radius {
                1.0
            } else if blend <= 0.0 || distance >= radius + blend {
                0.0
            } else {
                let t = (distance - radius) / blend;
                1.0 - t * t * (3.0 - 2.0 * t)
            };
            if weight > 0.0 {
                let h = &mut heights[j * width + i];
                *h += (level - *h) * weight;
            }
        }
    }
    with_tile(input, next)
}

fn inside(tile: &Tile, x: f32, z: f32) -> bool {
    let (w, d) = tile.inner_cells();
    let (x0, z0) = (
        tile.grid.world_x(tile.margin),
        tile.grid.world_z(tile.margin),
    );
    #[expect(clippy::cast_precision_loss, reason = "tiles are at most 2048 cells")]
    let (x1, z1) = (
        x0 + (w - 1) as f32 * tile.grid.cell(),
        z0 + (d - 1) as f32 * tile.grid.cell(),
    );
    (x0..=x1).contains(&x) && (z0..=z1).contains(&z)
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "positions are bounded metres in a grid of f32"
)]
fn place(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let (shape, ground) = match inputs {
        [shape, ground, ..] => (shape, ground),
        _ => {
            return Err(fail(
                "graph.missing_input",
                "terrain.place needs a shape and a terrain.tile",
            ));
        }
    };
    let tile = ground.grid.as_deref().ok_or_else(|| {
        fail_suggest(
            "terrain.needs_tile",
            "terrain.place needs the output of terrain.tile (or another terrain node) as its ground",
            "connect a terrain.tile, before solid.material, as the second input",
        )
    })?;
    let at = rows(node, "at", 2)?;
    let look = rows(node, "yaw_scale", 2)?;
    if !look.is_empty() && look.len() != at.len() {
        return Err(fail(
            "param.invalid",
            format!(
                "yaw_scale has {} entries but at lists {} positions; give one [yaw, scale] per position, or leave yaw_scale out",
                look.len(),
                at.len()
            ),
        ));
    }
    let (align, sink) = (param(node, "align")?, param(node, "sink")?);
    let base = shape.instances.clone().unwrap_or_else(|| InstanceSet {
        prototype: Rc::clone(shape),
        transforms: vec![DMat4::IDENTITY],
    });
    if at.len() * base.transforms.len() > MAX_INSTANCES {
        return Err(fail_suggest(
            "inst.unsatisfiable",
            format!(
                "{} placements is over the limit of {MAX_INSTANCES} per set",
                at.len() * base.transforms.len()
            ),
            "list fewer positions",
        ));
    }
    let mut transforms = Vec::with_capacity(at.len() * base.transforms.len());
    for (index, p) in at.iter().enumerate() {
        let (x, z) = (p[0] as f32, p[1] as f32);
        if !inside(tile, x, z) {
            return Err(fail_suggest(
                "terrain.outside_tile",
                format!(
                    "position [{}, {}] is not on the tile (it covers x {} to {}, z {} to {})",
                    p[0],
                    p[1],
                    tile.grid.world_x(tile.margin),
                    tile.grid.world_x(tile.grid.width() - tile.margin - 1),
                    tile.grid.world_z(tile.margin),
                    tile.grid.world_z(tile.grid.depth() - tile.margin - 1),
                ),
                "use a position inside the tile, or place on the neighbouring tile (tile_x, tile_z)",
            ));
        }
        let (yaw, scale) = look.get(index).map_or((0.0, 1.0), |l| (l[0], l[1]));
        if !(scale > 0.0) {
            return Err(fail("param.invalid", "yaw_scale scale must be above 0"));
        }
        let normal = tile.grid.normal(x, z);
        let up = DVec3::Y
            .lerp(
                DVec3::new(
                    f64::from(normal[0]),
                    f64::from(normal[1]),
                    f64::from(normal[2]),
                ),
                align,
            )
            .normalize_or_zero();
        let lean =
            DQuat::from_rotation_arc(DVec3::Y, if up == DVec3::ZERO { DVec3::Y } else { up });
        let stand = DMat4::from_scale_rotation_translation(
            DVec3::splat(scale),
            lean * DQuat::from_rotation_y(yaw.to_radians()),
            DVec3::new(
                f64::from(x),
                f64::from(tile.grid.sample(x, z)) - sink,
                f64::from(z),
            ),
        );
        for inner in &base.transforms {
            transforms.push(stand * *inner);
        }
    }
    let mut value = Value::nothing();
    value.material.clone_from(&base.prototype.material);
    value.volume = base.prototype.volume
        * transforms
            .iter()
            .map(|m| m.determinant().abs())
            .sum::<f64>();
    value.instances = Some(InstanceSet {
        prototype: base.prototype,
        transforms,
    });
    Ok(value)
}
