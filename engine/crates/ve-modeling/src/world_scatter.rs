//! `scatter.place`: copies of a shape planted over a terrain tile (ADR 0056, placement by `ve_world::scatter`).
//!
//! Placement is a pure function of the world seed and world coordinates, so tiles built separately never duplicate or drop a plant.
//! Where plants grow comes from a mask layer of the tile, a height window and a slope limit, folded into one density field.
use std::rc::Rc;

use glam::{DMat4, DQuat, DVec3};
use ve_graph::{Node, Value as Param};
use ve_kernel::ExecutionContext;
use ve_world::expr::Type;
use ve_world::scatter::{
    Field2, Poisson, ScatterError, Variation, align_to_surface, poisson, vary,
};

use crate::world_field::Pattern;

use crate::instances::{InstanceSet, MAX_INSTANCES};
use crate::world_terrain::{Tile, param};
use crate::{Fail, cache::Value, fail, fail_suggest};

pub(crate) fn run(
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Option<Result<Value, Fail>> {
    (node.op.family().as_str() == "scatter.place").then(|| place(node, inputs, context))
}

fn text<'a>(node: &'a Node, name: &str) -> &'a str {
    match node.params.get(name) {
        Some(Param::Str(value)) => value.as_str(),
        _ => "",
    }
}

fn scatter_error(error: ScatterError) -> Fail {
    match error {
        ScatterError::Cancelled => fail("eval.cancelled", "scatter cancelled"),
        ScatterError::TooMany { .. } => fail_suggest(
            "inst.unsatisfiable",
            error.to_string(),
            "raise `radius` or lower `density`",
        ),
        other => fail("scatter.failed", other.to_string()),
    }
}

/// The density at every cell of the tile's grid: the mask layer times the height window times the slope limit.
#[expect(
    clippy::cast_possible_truncation,
    reason = "parameters are bounded metres and degrees"
)]
fn density_field(node: &Node, tile: &Tile, wanted: Option<&Pattern>) -> Result<Vec<f32>, Fail> {
    let grid = &tile.grid;
    let layer = text(node, "layer");
    let mask = if layer.is_empty() {
        None
    } else {
        Some(grid.layer(layer).ok_or_else(|| {
            fail_suggest(
                "scatter.unknown_layer",
                format!(
                    "the tile has no layer called {layer:?} (it has {:?})",
                    grid.layer_names()
                ),
                "make it first with terrain.mask, before this node, and use the same name",
            )
        })?)
    };
    let (low, high) = (
        param(node, "min_height")? as f32,
        param(node, "max_height")? as f32,
    );
    // Plants thin out over the last 6 degrees below the limit instead of stopping at a contour line.
    let limit = param(node, "max_slope")? as f32;
    let (steepest, gentle) = (
        limit.to_radians().cos(),
        (limit - 6.0).max(0.0).to_radians().cos(),
    );
    let (w, d) = (grid.width(), grid.depth());
    let mut values = Vec::with_capacity(w * d);
    for j in 0..d {
        for i in 0..w {
            let index = j * w + i;
            let height = grid.heights()[index];
            let up = grid.normal(grid.world_x(i), grid.world_z(j))[1];
            let fit = if (steepest - gentle).abs() < 1e-6 {
                f32::from(up >= steepest - 1e-6)
            } else {
                let t = ((up - steepest) / (gentle - steepest)).clamp(0.0, 1.0);
                t * t * (3.0 - 2.0 * t)
            };
            let allowed = height >= low && height <= high;
            let pattern = match wanted {
                Some(pattern) if allowed => {
                    let (x, z) = (grid.world_x(i), grid.world_z(j));
                    pattern
                        .float([x, height, z], grid.normal(x, z))
                        .clamp(0.0, 1.0)
                }
                _ => 1.0,
            };
            values.push(if allowed {
                fit * mask.map_or(1.0, |layer| layer[index]) * pattern
            } else {
                0.0
            });
        }
    }
    Ok(values)
}

/// The slice `[low, high)` of 0 to 1 that this node's `pick` owns of its `mix`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "pick is a small bounded whole number"
)]
fn mix_slice(node: &Node) -> Result<(f64, f64), Fail> {
    let bad = || {
        fail_suggest(
            "scatter.bad_mix",
            format!("mix {:?} is not a list of positive numbers (at most 16) separated by commas", text(node, "mix")),
            "write the weights like `0.5, 0.3, 0.2`, or leave `mix` at 1 to plant everything with this node",
        )
    };
    let weights: Vec<f64> = text(node, "mix")
        .split(',')
        .map(|part| part.trim().parse::<f64>())
        .collect::<Result<_, _>>()
        .map_err(|_| bad())?;
    if weights.is_empty() || weights.len() > 16 || weights.iter().any(|w| !w.is_finite() || *w <= 0.0) {
        return Err(bad());
    }
    let pick = param(node, "pick")? as usize;
    if pick >= weights.len() {
        return Err(fail_suggest(
            "scatter.bad_pick",
            format!("pick {pick} is outside the {} shares of mix", weights.len()),
            "give `pick` a number from 0 to the number of weights in `mix` minus one",
        ));
    }
    let total: f64 = weights.iter().sum();
    let low = weights[..pick].iter().sum::<f64>() / total;
    Ok((low, low + weights[pick] / total))
}

/// A number in `[0, 1)` that depends only on where a plant stands and the seed, so every variant of one planting sees the same value for
/// the same plant and the shares of `mix` never overlap, whichever tile is built first.
fn share_of(position: [f32; 3], seed: u32) -> f64 {
    let mut state = u64::from(seed).wrapping_add(0x9e37_79b9_7f4a_7c15);
    for coordinate in [position[0], position[2]] {
        state ^= u64::from(coordinate.to_bits());
        state = state.wrapping_mul(0xff51_afd7_ed55_8ccd);
        state ^= state >> 33;
        state = state.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
        state ^= state >> 29;
    }
    #[expect(clippy::cast_precision_loss, reason = "53 bits fill an f64 mantissa exactly")]
    let fraction = (state >> 11) as f64 / (1_u64 << 53) as f64;
    fraction
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "parameters are bounded and the counts are small"
)]
fn place(node: &Node, inputs: &[Rc<Value>], context: &ExecutionContext) -> Result<Value, Fail> {
    let show_from = param(node, "show_from")? as f32;
    let show_to = param(node, "show_to")? as f32;
    let show_fade = param(node, "show_fade")? as f32;
    if show_to != 0.0 && show_to <= show_from {
        return Err(fail_suggest(
            "scatter.invalid_show_range",
            "show_to must be 0 (no limit) or greater than show_from",
            "raise `show_to` above `show_from`, or set it to 0 for no limit",
        ));
    }
    if show_to != 0.0 && show_fade > (show_to - show_from) * 0.5 {
        return Err(fail_suggest(
            "scatter.invalid_show_fade",
            "show_fade cannot be greater than half the show_from/show_to band width",
            "lower `show_fade` to at most half the distance between `show_from` and `show_to`",
        ));
    }
    let (shape, ground) = match inputs {
        [shape, ground, ..] => (shape, ground),
        _ => {
            return Err(fail(
                "graph.missing_input",
                "scatter.place needs a shape and a terrain.tile",
            ));
        }
    };
    let tile = ground.grid.as_deref().ok_or_else(|| {
        fail_suggest(
            "terrain.needs_tile",
            "scatter.place needs the output of terrain.tile (or another terrain node) as its ground",
            "connect a terrain.tile, before solid.material, as the second input",
        )
    })?;
    let base = shape.instances.clone().unwrap_or_else(|| InstanceSet {
        prototype: Rc::clone(shape),
        transforms: vec![DMat4::IDENTITY],
    });
    let wanted = inputs.get(2).and_then(|value| value.pattern.as_deref());
    if let Some(pattern) = wanted {
        if pattern.output_type() != Type::Float {
            return Err(fail_suggest(
                "pattern.not_float",
                format!(
                    "the `where` pattern of {} gives {:?}, but a density needs one number",
                    node.id,
                    pattern.output_type()
                ),
                "end the expression with a single number, for example `out = smoothstep(0.2, 0.6, n.y)`, or pick one component such as `.x`",
            ));
        }
    }
    let values = density_field(node, tile, wanted)?;
    let grid = &tile.grid;
    let (width, depth, cell, origin) = (grid.width(), grid.depth(), grid.cell(), grid.origin());
    let mask = Field2 {
        values: &values,
        width,
        depth,
        cell,
        origin,
    };
    let heights = Field2 {
        values: grid.heights(),
        width,
        depth,
        cell,
        origin,
    };
    let (w, d) = tile.inner_cells();
    let (x0, z0) = (grid.world_x(tile.margin), grid.world_z(tile.margin));
    #[expect(
        clippy::cast_precision_loss,
        reason = "tile sizes are at most 2048 cells"
    )]
    let area = [
        x0,
        z0,
        x0 + (w - 1) as f32 * grid.cell(),
        z0 + (d - 1) as f32 * grid.cell(),
    ];
    let seed = param(node, "seed")? as u32;
    let cancel = || context.is_cancelled();
    let settings = Poisson {
        seed,
        radius: param(node, "radius")? as f32,
        density: param(node, "density")? as f32,
        area,
        tries: 8,
    };
    let mut points =
        poisson(&settings, Some(&mask), Some(&heights), &cancel).map_err(scatter_error)?;
    let scale = match node.params.get("scale") {
        Some(Param::List(items)) if items.len() == 2 => [
            items[0].as_f64().unwrap_or(1.0) as f32,
            items[1].as_f64().unwrap_or(1.0) as f32,
        ],
        _ => [0.8, 1.2],
    };
    vary(
        &mut points,
        &Variation {
            seed,
            scale: [scale[0].min(scale[1]), scale[0].max(scale[1])],
            species: 0,
            age: [0.0, 1.0],
        },
    );
    align_to_surface(&mut points, &heights, 1.0);
    if points.len() * base.transforms.len() > MAX_INSTANCES {
        return Err(fail_suggest(
            "inst.unsatisfiable",
            format!(
                "{} placements is over the limit of {MAX_INSTANCES} per set",
                points.len() * base.transforms.len()
            ),
            "raise `radius`, lower `density`, or use smaller tiles",
        ));
    }
    let (align, sink, spin) = (
        param(node, "align")?,
        param(node, "sink")?,
        !matches!(node.params.get("spin"), Some(Param::Bool(false))),
    );
    let (scales, yaws, normals) = (
        points.channel("scale"),
        points.channel("yaw"),
        points.vector_channel("normal"),
    );
    let (slice_low, slice_high) = mix_slice(node)?;
    let mut transforms = Vec::with_capacity(points.len() * base.transforms.len());
    for (k, position) in points.positions().iter().enumerate() {
        let share = share_of(*position, seed);
        if share < slice_low || share >= slice_high {
            continue;
        }
        let normal = normals.and_then(|n| n.get(k)).map_or(DVec3::Y, |n| {
            DVec3::new(f64::from(n[0]), f64::from(n[1]), f64::from(n[2]))
        });
        let up = DVec3::Y.lerp(normal, align).normalize_or_zero();
        let lean =
            DQuat::from_rotation_arc(DVec3::Y, if up == DVec3::ZERO { DVec3::Y } else { up });
        let yaw = if spin {
            yaws.and_then(|y| y.get(k)).map_or(0.0, |y| f64::from(*y))
        } else {
            0.0
        };
        let size = scales.and_then(|s| s.get(k)).map_or(1.0, |s| f64::from(*s));
        let place = DMat4::from_scale_rotation_translation(
            DVec3::splat(size),
            lean * DQuat::from_rotation_y(yaw),
            DVec3::new(
                f64::from(position[0]),
                f64::from(position[1]) - sink,
                f64::from(position[2]),
            ),
        );
        for inner in &base.transforms {
            transforms.push(place * *inner);
        }
    }
    let mut value = Value::nothing();
    if transforms.is_empty() {
        // A tile with no suitable ground (no steep slopes for rocks, no low ground for reeds) is ordinary in a world made of tiles:
        // it gets no plants and a warning, not an error that would stop every other tile.
        value.warnings.push(crate::EvaluationWarning {
            code: "scatter.nothing".to_owned(),
            node: node.id.to_string(),
            related: Vec::new(),
            message: "nothing was planted: the mask layer, height window and slope limit leave no ground on this tile; widen min_height, max_height or max_slope, or lower `radius` if that is not expected".to_owned(),
        });
        return Ok(value);
    }
    value.material.clone_from(&base.prototype.material);
    value.volume = base.prototype.volume
        * transforms
            .iter()
            .map(|m| m.determinant().abs())
            .sum::<f64>();
    let mut prototype = (*base.prototype).clone();
    prototype.visible_fade = show_fade;
    if show_from != 0.0 || show_to != 0.0 {
        prototype.visible_range = Some((
            show_from,
            if show_to == 0.0 {
                f32::INFINITY
            } else {
                show_to
            },
        ));
    }
    value.instances = Some(InstanceSet {
        prototype: Rc::new(prototype),
        transforms,
    });
    Ok(value)
}
