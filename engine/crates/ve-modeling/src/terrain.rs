//! `terrain.heightfield`: a seeded fractal landscape as a closed solid slab (hills, mountains, islands, lake beds).
//!
//! The top is a regular grid of heights from fractal value noise (`crate::noise`), the sides drop straight down to a flat
//! base, so the result is an ordinary watertight solid: it can be cut, painted, scattered over and exported like any other.
#![expect(
    clippy::cast_precision_loss,
    reason = "grid indices are at most 192, far below 2^52"
)]
use crate::{
    Fail,
    cache::Value,
    fail,
    noise::fractal,
    operations::kernel_fail,
    profile_ops::{number, pair, whole},
};
use std::rc::Rc;
use ve_graph::{Node, Value as Param};
use ve_kernel::{ExecutionContext, Mesh, Solid};

#[path = "water.rs"]
pub(crate) mod water;

/// Most grid cells along one side.
pub(crate) const MAX_RESOLUTION: u32 = 192;

pub(crate) fn run(
    node: &Node,
    _inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Option<Result<Value, Fail>> {
    (node.op.family().as_str() == "terrain.heightfield").then(|| build(node, context))
}

/// Triangles the node will make (for the cost estimate): top, four walls and the base.
pub(crate) fn estimate(node: &Node) -> usize {
    let cells = whole(node, "resolution")
        .unwrap_or(64)
        .clamp(4, MAX_RESOLUTION) as usize;
    2 * cells * cells + 8 * cells + 4 * cells
}

struct Shape {
    size: [f64; 2],
    height: f64,
    scale: f64,
    octaves: u32,
    roughness: f64,
    seed: u32,
    ridged: bool,
    edge_falloff: f64,
    flat_below: f64,
}

impl Shape {
    fn from_node(node: &Node) -> Result<Self, Fail> {
        let bad = |what: &str| {
            fail(
                "param.type",
                format!("terrain.heightfield {what} is missing or malformed"),
            )
        };
        Ok(Self {
            size: pair(node, "size").map_err(|_| bad("size"))?,
            height: number(node, "height").map_err(|_| bad("height"))?,
            scale: number(node, "scale").map_err(|_| bad("scale"))?,
            octaves: whole(node, "octaves")
                .map_err(|_| bad("octaves"))?
                .clamp(1, 6),
            roughness: number(node, "roughness").map_err(|_| bad("roughness"))?,
            seed: whole(node, "seed").map_err(|_| bad("seed"))?,
            ridged: matches!(node.params.get("style"), Some(Param::Str(name)) if name == "ridged"),
            edge_falloff: number(node, "edge_falloff")
                .map_err(|_| bad("edge_falloff"))?
                .clamp(0.0, 1.0),
            flat_below: number(node, "flat_below")
                .map_err(|_| bad("flat_below"))?
                .clamp(0.0, 0.95),
        })
    }

    /// Ground height above the base plane at `(x, z)`, in `0..=height`.
    fn relief(&self, x: f64, z: f64) -> f64 {
        let n = fractal(
            [x / self.scale, 0.371, z / self.scale],
            self.seed,
            self.octaves,
            self.roughness,
        );
        let mut t = if self.ridged {
            let ridge = 1.0 - n.abs();
            ridge * ridge
        } else {
            (n + 1.0) * 0.5
        };
        if self.edge_falloff > 0.0 {
            let reach = self.edge_falloff * (self.size[0].min(self.size[1]) * 0.5);
            let inside = ((self.size[0] * 0.5 - x.abs()).min(self.size[1] * 0.5 - z.abs()) / reach)
                .clamp(0.0, 1.0);
            t *= inside * inside * (3.0 - 2.0 * inside);
        }
        self.height * t.max(self.flat_below)
    }
}

fn build(node: &Node, context: &ExecutionContext) -> Result<Value, Fail> {
    let shape = Shape::from_node(node)?;
    let cells = whole(node, "resolution")
        .map_err(|_| fail("param.type", "terrain.heightfield resolution is missing"))?;
    let cells = cells.clamp(4, MAX_RESOLUTION) as usize;
    let base = number(node, "base")
        .map_err(|_| fail("param.type", "terrain.heightfield base is missing"))?;
    let side = cells + 1;
    let at = |i: usize, j: usize| {
        (
            shape.size[0] * (i as f64 / cells as f64 - 0.5),
            shape.size[1] * (j as f64 / cells as f64 - 0.5),
        )
    };
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(side * side + 8 * cells + 1);
    for j in 0..side {
        if context.is_cancelled() {
            return Err(fail("eval.cancelled", "terrain generation cancelled"));
        }
        for i in 0..side {
            let (x, z) = at(i, j);
            positions.push(to_f32([x, shape.relief(x, z), z]));
        }
    }
    let top = |i: usize, j: usize| u32::try_from(j * side + i).unwrap_or(0);
    let mut indices: Vec<u32> = Vec::with_capacity(6 * cells * cells + 18 * cells * 2);
    for j in 0..cells {
        for i in 0..cells {
            let (a, b, c, d) = (top(i, j), top(i, j + 1), top(i + 1, j), top(i + 1, j + 1));
            indices.extend_from_slice(&[a, b, c, b, d, c]);
        }
    }
    // The boundary loop of the top grid, walked so the interior is on one side.
    let mut loop_cells: Vec<(usize, usize)> = Vec::with_capacity(4 * cells);
    loop_cells.extend((0..cells).map(|j| (0, j)));
    loop_cells.extend((0..cells).map(|i| (i, cells)));
    loop_cells.extend((0..cells).map(|j| (cells, cells - j)));
    loop_cells.extend((0..cells).map(|i| (cells - i, 0)));
    let first_bottom = u32::try_from(positions.len())
        .map_err(|_| fail("budget.triangles", "terrain is too large"))?;
    for (i, j) in &loop_cells {
        let (x, z) = at(*i, *j);
        positions.push(to_f32([x, -base, z]));
    }
    let centre = u32::try_from(positions.len())
        .map_err(|_| fail("budget.triangles", "terrain is too large"))?;
    positions.push([0.0, to_f32([0.0, -base, 0.0])[1], 0.0]);
    let count = loop_cells.len();
    for k in 0..count {
        let next = (k + 1) % count;
        let (t0, t1) = (
            top(loop_cells[k].0, loop_cells[k].1),
            top(loop_cells[next].0, loop_cells[next].1),
        );
        let (b0, b1) = (
            first_bottom + u32::try_from(k).unwrap_or(0),
            first_bottom + u32::try_from(next).unwrap_or(0),
        );
        indices.extend_from_slice(&[t0, b0, b1, t0, b1, t1]);
        indices.extend_from_slice(&[centre, b1, b0]);
    }
    let mut mesh = Mesh { positions, indices };
    if signed_volume(&mesh) < 0.0 {
        for triangle in mesh.indices.chunks_exact_mut(3) {
            triangle.swap(1, 2);
        }
    }
    if context.is_cancelled() {
        return Err(fail("eval.cancelled", "terrain generation cancelled"));
    }
    let solid = Solid::from_mesh(&mesh).map_err(|error| kernel_fail(node, &error))?;
    let mesh = solid
        .to_mesh()
        .map_err(|error| fail("kernel.failed", error.to_string()))?;
    Ok(Value::solid(mesh, solid.volume()))
}

fn to_f32(point: [f64; 3]) -> [f32; 3] {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "terrain coordinates are bounded metres"
    )]
    point.map(|v| v as f32)
}

/// Volume by the divergence theorem; negative when the triangles wind inward.
fn signed_volume(mesh: &Mesh) -> f64 {
    let mut total = 0.0;
    for triangle in mesh.indices.chunks_exact(3) {
        let [a, b, c] = [0, 1, 2].map(|k| mesh.positions[triangle[k] as usize].map(f64::from));
        total += (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
            + a[2] * (b[0] * c[1] - b[1] * c[0]))
            / 6.0;
    }
    total
}
