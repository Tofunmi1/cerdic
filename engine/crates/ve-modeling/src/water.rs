//! `water.surface`: a deterministic closed water slab with a gridded, gently varied rest surface.
use crate::{
    Fail, Material, MaterialRecipe,
    cache::Value,
    fail,
    noise::fractal,
    operations::kernel_fail,
    profile_ops::{number, pair, whole},
};
use std::rc::Rc;
use ve_graph::Node;
use ve_kernel::{ExecutionContext, Mesh, Solid};

const MAX_RESOLUTION: u32 = 192;

struct WaterParams {
    size: [f64; 2],
    level: f64,
    depth: f64,
    wave_height: f64,
    wave_length: f64,
    seed: u32,
    cells: usize,
    opacity: f32,
    color: [f32; 3],
    ripple_speed: f32,
}

pub(crate) fn run(
    node: &Node,
    _inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Option<Result<Value, Fail>> {
    (node.op.family().as_str() == "water.surface").then(|| build(node, context))
}

pub(crate) fn estimate(node: &Node) -> usize {
    let cells = whole(node, "resolution")
        .unwrap_or(64)
        .clamp(4, MAX_RESOLUTION) as usize;
    2 * cells * cells + 8 * cells + 4 * cells
}

fn build(node: &Node, context: &ExecutionContext) -> Result<Value, Fail> {
    let params = parameters(node)?;
    let mesh = make_mesh(&params, context)?;
    let solid = Solid::from_mesh(&mesh).map_err(|error| kernel_fail(node, &error))?;
    let mut value = Value::solid(
        solid
            .to_mesh()
            .map_err(|error| fail("kernel.failed", error.to_string()))?,
        solid.volume(),
    );
    // Lakes have no current or shore foam; depth uses the river shader's units of six metres.
    let depth01 = f32_point([0.0, params.depth / 6.0, 0.0])[1].clamp(0.0, 1.0);
    value.colors = vec![[0.5, 0.5, depth01, 0.0]; value.mesh.positions.len()];
    value.material = Some(Material {
        color: params.color,
        opacity: params.opacity,
        metallic: 0.0,
        roughness: 0.08,
        emissive: [0.0; 3],
        emits_light: false,
        recipe: MaterialRecipe::Water,
        texture_scale: params.ripple_speed,
        texture_tile: 0.0,
        no_repeat: false,
        no_repeat_scale: 1.0,
        texture_size: 0,
        toon: None,
        texture_program: None,
        texture_style: None,
        foliage: None,
        finish: Some(crate::finish::Finish {
            water: Some(crate::water_look::lake_look(
                node,
                params.color,
                params.ripple_speed,
            )?),
            ..crate::finish::Finish::plain()
        }),
    });
    Ok(value)
}

fn parameters(node: &Node) -> Result<WaterParams, Fail> {
    let size = pair(node, "size").map_err(|_| {
        fail(
            "param.type",
            "water.surface size must be two positive lengths",
        )
    })?;
    let level =
        number(node, "level").map_err(|_| fail("param.type", "water.surface level is missing"))?;
    let depth =
        number(node, "depth").map_err(|_| fail("param.type", "water.surface depth is missing"))?;
    let wave_height = number(node, "wave_height")
        .map_err(|_| fail("param.type", "water.surface wave_height is missing"))?;
    let wave_length = number(node, "wave_length")
        .map_err(|_| fail("param.type", "water.surface wave_length is missing"))?;
    let seed =
        whole(node, "seed").map_err(|_| fail("param.type", "water.surface seed is missing"))?;
    let cells = whole(node, "resolution")
        .map_err(|_| fail("param.type", "water.surface resolution is missing"))?
        .clamp(4, MAX_RESOLUTION) as usize;
    let opacity = number(node, "opacity")
        .map_err(|_| fail("param.type", "water.surface opacity is missing"))?;
    let color = colour(node)?;
    let ripple_rate = number(node, "speed").unwrap_or(1.0);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "opacity is bounded to 0..=1 by the graph spec"
    )]
    let opacity = opacity as f32;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "speed is bounded to 0..=10 by the graph spec"
    )]
    let ripple_speed = ripple_rate as f32;
    Ok(WaterParams {
        size,
        level,
        depth,
        wave_height,
        wave_length,
        seed,
        cells,
        opacity,
        color,
        ripple_speed,
    })
}

fn make_mesh(params: &WaterParams, context: &ExecutionContext) -> Result<Mesh, Fail> {
    if context.is_cancelled() {
        return Err(fail("eval.cancelled", "water generation cancelled"));
    }
    let (positions, indices) = make_top_grid(params, context)?;
    close_slab(params, positions, indices)
}

fn make_top_grid(
    params: &WaterParams,
    context: &ExecutionContext,
) -> Result<(Vec<[f32; 3]>, Vec<u32>), Fail> {
    let grid_width = params.cells + 1;
    let mut positions = Vec::with_capacity(grid_width * grid_width + 8 * params.cells + 1);
    for j in 0..grid_width {
        if context.is_cancelled() {
            return Err(fail("eval.cancelled", "water generation cancelled"));
        }
        for i in 0..grid_width {
            #[expect(
                clippy::cast_precision_loss,
                reason = "water grid is capped at 192 cells"
            )]
            let (u, v) = (
                i as f64 / params.cells as f64 - 0.5,
                j as f64 / params.cells as f64 - 0.5,
            );
            let (x, z) = (u * params.size[0], v * params.size[1]);
            let noise = fractal(
                [x / params.wave_length, 0.371, z / params.wave_length],
                params.seed,
                3,
                0.45,
            );
            positions.push(f32_point([x, params.level + noise * params.wave_height, z]));
        }
    }
    let mut indices = Vec::with_capacity(6 * params.cells * params.cells + 18 * params.cells);
    for j in 0..params.cells {
        for i in 0..params.cells {
            let (a, b, c, d) = (
                top_index(i, j, grid_width),
                top_index(i, j + 1, grid_width),
                top_index(i + 1, j, grid_width),
                top_index(i + 1, j + 1, grid_width),
            );
            indices.extend_from_slice(&[a, b, c, b, d, c]);
        }
    }
    Ok((positions, indices))
}

fn close_slab(
    params: &WaterParams,
    mut positions: Vec<[f32; 3]>,
    mut indices: Vec<u32>,
) -> Result<Mesh, Fail> {
    let mut boundary = Vec::with_capacity(4 * params.cells);
    boundary.extend((0..params.cells).map(|j| (0, j)));
    boundary.extend((0..params.cells).map(|i| (i, params.cells)));
    boundary.extend((0..params.cells).map(|j| (params.cells, params.cells - j)));
    boundary.extend((0..params.cells).map(|i| (params.cells - i, 0)));
    let bottom_start = u32::try_from(positions.len())
        .map_err(|_| fail("budget.triangles", "water mesh is too large"))?;
    for &(i, j) in &boundary {
        #[expect(
            clippy::cast_precision_loss,
            reason = "water grid is capped at 192 cells"
        )]
        let (x, z) = (
            (i as f64 / params.cells as f64 - 0.5) * params.size[0],
            (j as f64 / params.cells as f64 - 0.5) * params.size[1],
        );
        positions.push(f32_point([x, params.level - params.depth, z]));
    }
    let centre = u32::try_from(positions.len())
        .map_err(|_| fail("budget.triangles", "water mesh is too large"))?;
    positions.push(f32_point([0.0, params.level - params.depth, 0.0]));
    for k in 0..boundary.len() {
        let next = (k + 1) % boundary.len();
        let (a, b) = (boundary[k], boundary[next]);
        let (t0, t1) = (
            top_index(a.0, a.1, params.cells + 1),
            top_index(b.0, b.1, params.cells + 1),
        );
        let k =
            u32::try_from(k).map_err(|_| fail("budget.triangles", "water mesh is too large"))?;
        let next =
            u32::try_from(next).map_err(|_| fail("budget.triangles", "water mesh is too large"))?;
        let (b0, b1) = (bottom_start + k, bottom_start + next);
        indices.extend_from_slice(&[t0, b0, b1, t0, b1, t1]);
        indices.extend_from_slice(&[centre, b1, b0]);
    }
    let mut mesh = Mesh { positions, indices };
    if signed_volume(&mesh) < 0.0 {
        for tri in mesh.indices.chunks_exact_mut(3) {
            tri.swap(1, 2);
        }
    }
    Ok(mesh)
}

fn top_index(i: usize, j: usize, grid_width: usize) -> u32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the grid is capped at 193 by 193 vertices"
    )]
    let index = (j * grid_width + i) as u32;
    index
}

fn colour(node: &Node) -> Result<[f32; 3], Fail> {
    let Some(ve_graph::Value::List(values)) = node.params.get("color") else {
        return Err(fail(
            "param.type",
            "water.surface color must be an RGB triple",
        ));
    };
    let [r, g, b] = values.as_slice() else {
        return Err(fail(
            "param.type",
            "water.surface color must be an RGB triple",
        ));
    };
    let channels = [r.as_f64(), g.as_f64(), b.as_f64()];
    if channels
        .iter()
        .any(|v| !v.is_some_and(|n| n.is_finite() && (0.0..=1.0).contains(&n)))
    {
        return Err(fail(
            "param.range",
            "water.surface color channels must be between zero and one",
        ));
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "RGB channels are bounded to 0..=1"
    )]
    Ok(channels.map(|v| v.unwrap_or(0.0) as f32))
}

fn f32_point(point: [f64; 3]) -> [f32; 3] {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "water extents are bounded at the graph boundary"
    )]
    point.map(|value| value as f32)
}

fn signed_volume(mesh: &Mesh) -> f64 {
    mesh.indices
        .chunks_exact(3)
        .map(|tri| {
            let [a, b, c] = [0, 1, 2].map(|i| mesh.positions[tri[i] as usize].map(f64::from));
            (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                + a[2] * (b[0] * c[1] - b[1] * c[0]))
                / 6.0
        })
        .sum()
}
