//! Volume grids: a closed mesh sampled into a grid of signed distances, combined, smoothed, grown or shrunk as numbers, and turned back into
//! a mesh. Built here, no outside library: the distance field is made from exact point-to-triangle distances with a sign from angle-weighted
//! pseudo-normals (Bærentzen and Aanæs), and the mesh comes from surface nets. Everything is `+ - * /` and `sqrt` except one `atan2` that
//! weights a vertex normal (see `sdf.rs`), so a volume is the same on every platform apart from a last-bit tie in a sign.
//!
//! The grids are truncated: a voxel farther from the surface than `band` cells holds plus or minus that distance (negative inside). All grids
//! sit on one world lattice (their origins are whole multiples of the cell size), so grids of the same cell size combine without resampling.

mod sdf;
mod surface_nets;

use std::rc::Rc;

use ve_graph::{Node, Value as Param};
use ve_kernel::ExecutionContext;

use crate::cache::Value;
use crate::{Fail, fail, fail_suggest};

/// The most voxels one grid may have (24 MB of distances).
pub(crate) const MAX_VOXELS: usize = 6_000_000;

/// A truncated signed-distance grid on the world lattice.
pub(crate) struct Volume {
    /// World position of voxel (0, 0, 0): a whole multiple of `cell` on each axis.
    pub(crate) origin: [f64; 3],
    /// Voxel size in metres.
    pub(crate) cell: f64,
    /// Voxels along x, y and z.
    pub(crate) dims: [usize; 3],
    /// Signed distance per voxel, x fastest then y then z; negative inside, clamped to plus or minus `trunc`.
    pub(crate) values: Vec<f32>,
    /// The distance beyond which values are clamped, in metres.
    pub(crate) trunc: f32,
}

impl Volume {
    pub(crate) fn bytes(&self) -> usize {
        self.values.capacity() * size_of::<f32>() + size_of::<Self>()
    }

    pub(crate) fn index(&self, i: usize, j: usize, k: usize) -> usize {
        i + self.dims[0] * (j + self.dims[1] * k)
    }
}

fn number(node: &Node, name: &str) -> Result<f64, Fail> {
    node.params
        .get(name)
        .and_then(Param::as_f64)
        .ok_or_else(|| fail("param.type", format!("{name} is missing or not a number")))
}

fn text<'a>(node: &'a Node, name: &str) -> Result<&'a str, Fail> {
    match node.params.get(name) {
        Some(Param::Str(value)) => Ok(value),
        _ => Err(fail("param.type", format!("{name} is missing or not text"))),
    }
}

fn whole(node: &Node, name: &str) -> Result<usize, Fail> {
    let value = number(node, name)?;
    if value < 0.0 || value.fract() != 0.0 {
        return Err(fail("param.type", format!("{name} must be a whole number")));
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "checked above"
    )]
    Ok(value as usize)
}

fn volume_value(volume: Volume) -> Value {
    let mut value = Value::nothing();
    value.sdf = Some(Rc::new(volume));
    value
}

fn input_volume<'a>(
    node: &Node,
    inputs: &'a [Rc<Value>],
    at: usize,
) -> Result<&'a Rc<crate::volume::Volume>, Fail> {
    inputs.get(at).and_then(|v| v.sdf.as_ref()).ok_or_else(|| {
        fail_suggest(
            "volume.not_a_volume",
            format!("{} needs a volume on its input", node.op),
            "make one from a closed mesh with volume.from_mesh",
        )
    })
}

pub(crate) fn run(
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Option<Result<Value, Fail>> {
    let family = node.op.family().as_str();
    if !family.starts_with("volume.") {
        return None;
    }
    Some(match family {
        "volume.from_mesh" => from_mesh(node, inputs),
        "volume.combine" => combine(node, inputs),
        "volume.smooth" => smooth(node, inputs),
        "volume.offset" => offset(node, inputs),
        "volume.to_mesh" => surface_nets::to_mesh(node, inputs, context),
        _ => Err(fail(
            "graph.unknown_op",
            format!("{family} is not a volume node"),
        )),
    })
}

/// A closed mesh sampled into a distance grid, for other nodes that need one (a collider for cloth). See `volume.from_mesh`.
/// The surface of `volume` at level `iso` as a closed polygon value, by surface nets.
pub(crate) fn mesh_of(
    node: &Node,
    volume: &Volume,
    iso: f32,
    context: &ExecutionContext,
) -> Result<Value, Fail> {
    surface_nets::mesh_volume(node, volume, iso, context)
}

pub(crate) fn sample_mesh(
    node: &Node,
    mesh: &ve_kernel::Mesh,
    voxel_size: f64,
    band: usize,
) -> Result<Volume, Fail> {
    sdf::build(node, mesh, voxel_size, band)
}

fn from_mesh(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = inputs.first().ok_or_else(|| {
        fail(
            "graph.missing_input",
            "volume.from_mesh needs a closed mesh",
        )
    })?;
    sdf::build(
        node,
        &input.mesh,
        number(node, "voxel_size")?,
        whole(node, "band")?,
    )
    .map(volume_value)
}

/// Joins the grids: union, subtract (the first minus the rest) or intersect, optionally blended over `blend` metres.
fn combine(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let grids: Vec<&Rc<Volume>> = (0..inputs.len())
        .map(|i| input_volume(node, inputs, i))
        .collect::<Result<_, _>>()?;
    let Some(first) = grids.first() else {
        return Err(fail(
            "graph.missing_input",
            "volume.combine needs at least two volumes",
        ));
    };
    let cell = first.cell;
    if grids.iter().any(|g| (g.cell - cell).abs() > 1e-12 * cell) {
        return Err(fail_suggest(
            "volume.cell_mismatch",
            format!(
                "the volumes have different voxel sizes ({}); they can only be combined at one size",
                grids
                    .iter()
                    .map(|g| format!("{:.4}", g.cell))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            "give every volume.from_mesh the same voxel_size",
        ));
    }
    let trunc = grids.iter().map(|g| g.trunc).fold(f32::INFINITY, f32::min);
    // The box that holds them all, in whole voxels from the lowest origin.
    let mut low = first.origin;
    let mut high = [0.0_f64; 3];
    for g in &grids {
        for axis in 0..3 {
            low[axis] = low[axis].min(g.origin[axis]);
            #[expect(clippy::cast_precision_loss, reason = "dimensions are far below 2^53")]
            let top = g.origin[axis] + g.dims[axis] as f64 * cell;
            high[axis] = high[axis].max(top);
        }
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a positive box measured in whole voxels"
    )]
    let dims = [0, 1, 2].map(|a| ((high[a] - low[a]) / cell).round() as usize);
    let total = dims[0].saturating_mul(dims[1]).saturating_mul(dims[2]);
    if total > MAX_VOXELS {
        return Err(too_big(total, cell));
    }
    let blend = number(node, "blend")? as f32;
    let op = text(node, "operation")?;
    let mut values = vec![trunc; total];
    let mut first_pass = true;
    for g in &grids {
        let offset = [0, 1, 2].map(|a| ((g.origin[a] - low[a]) / cell).round());
        if (0..3).any(|a| (g.origin[a] - low[a] - offset[a] * cell).abs() > 1e-6 * cell) {
            return Err(fail_suggest(
                "volume.lattice_mismatch",
                "the volumes are not on the same lattice of voxels".to_owned(),
                "make them all with volume.from_mesh at the same voxel_size",
            ));
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "offsets are small whole numbers"
        )]
        let offset = offset.map(|o| o as usize);
        // What this grid says at each voxel of the combined box: its value inside its own box, plus (outside) beyond it.
        let mut incoming = vec![g.trunc; total];
        for k in 0..g.dims[2] {
            for j in 0..g.dims[1] {
                let from = g.index(0, j, k);
                let to = (offset[0]) + dims[0] * ((j + offset[1]) + dims[1] * (k + offset[2]));
                incoming[to..to + g.dims[0]].copy_from_slice(&g.values[from..from + g.dims[0]]);
            }
        }
        if first_pass {
            values.copy_from_slice(&incoming);
            first_pass = false;
            continue;
        }
        for (v, b) in values.iter_mut().zip(&incoming) {
            *v = match op {
                "subtract" => smooth_max(*v, -*b, blend),
                "intersect" => smooth_max(*v, *b, blend),
                _ => smooth_min(*v, *b, blend),
            }
            .clamp(-trunc, trunc);
        }
    }
    Ok(volume_value(Volume {
        origin: low,
        cell,
        dims,
        values,
        trunc,
    }))
}

fn smooth_min(a: f32, b: f32, k: f32) -> f32 {
    if k <= 0.0 {
        return a.min(b);
    }
    let h = (k - (a - b).abs()).max(0.0) / k;
    a.min(b) - h * h * k * 0.25
}

fn smooth_max(a: f32, b: f32, k: f32) -> f32 {
    -smooth_min(-a, -b, k)
}

fn too_big(voxels: usize, cell: f64) -> Fail {
    fail_suggest(
        "budget.volume",
        format!(
            "the grid would have {voxels} voxels (the most is {MAX_VOXELS}) at a voxel size of {cell:.4} m"
        ),
        "raise voxel_size, or make the shape smaller",
    )
}

/// Evens the grid out: each pass moves every value toward the average of its six neighbours.
fn smooth(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = input_volume(node, inputs, 0)?;
    let (passes, factor) = (whole(node, "iterations")?, number(node, "factor")? as f32);
    let [nx, ny, nz] = input.dims;
    let mut values = input.values.clone();
    let mut next = values.clone();
    for _ in 0..passes {
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let at = i + nx * (j + ny * k);
                    let v = values[at];
                    let sample = |di: isize, dj: isize, dk: isize| {
                        let (a, b, c) = (
                            (i as isize + di).clamp(0, nx as isize - 1) as usize,
                            (j as isize + dj).clamp(0, ny as isize - 1) as usize,
                            (k as isize + dk).clamp(0, nz as isize - 1) as usize,
                        );
                        values[a + nx * (b + ny * c)]
                    };
                    let mean = (sample(-1, 0, 0)
                        + sample(1, 0, 0)
                        + sample(0, -1, 0)
                        + sample(0, 1, 0)
                        + sample(0, 0, -1)
                        + sample(0, 0, 1))
                        / 6.0;
                    next[at] = v + (mean - v) * factor;
                }
            }
        }
        std::mem::swap(&mut values, &mut next);
    }
    Ok(volume_value(Volume {
        values,
        origin: input.origin,
        cell: input.cell,
        dims: input.dims,
        trunc: input.trunc,
    }))
}

/// Grows (positive distance) or shrinks (negative) the shape by moving the zero level.
fn offset(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = input_volume(node, inputs, 0)?;
    let distance = number(node, "distance")?;
    let limit = f64::from(input.trunc) * 0.9;
    if distance.abs() > limit {
        return Err(fail_suggest(
            "volume.offset_range",
            format!(
                "an offset of {distance} m is more than the grid can hold: it keeps distances up to {:.4} m from the surface (band times voxel size)",
                input.trunc
            ),
            "use a smaller distance, or make the volume with a larger band",
        ));
    }
    let shift = distance as f32;
    let values = input
        .values
        .iter()
        .map(|v| (v - shift).clamp(-input.trunc, input.trunc))
        .collect();
    Ok(volume_value(Volume {
        values,
        origin: input.origin,
        cell: input.cell,
        dims: input.dims,
        trunc: input.trunc,
    }))
}
