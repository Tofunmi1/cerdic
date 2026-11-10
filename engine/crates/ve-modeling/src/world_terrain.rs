//! `terrain.tile`: one tile of a world's ground as an editable polygon mesh over a height grid (ADR 0056, built by `ve_world::terrain`).
//!
//! The grid is evaluated at world coordinates, so tiles with the same seed meet exactly; the mesh is its quads, with corner UVs across the
//! tile, so every `poly.*` node and `solid.material` take it like any other polygon mesh while `terrain.*` nodes read the grid behind it.
//! A tile keeps a margin of extra cells around the part that is meshed, and remembers how it was generated (its [`TerrainSource`]), so
//! masks that look around (occlusion) and erosion that moves material across edges can be computed without a seam between tiles.
use std::rc::Rc;

use ve_geometry::{AttrData, Attribute, Domain, PolyMesh, TriangulationMode};
use ve_graph::{Node, Value as Param};
use ve_kernel::{ExecutionContext, Mesh};
use ve_world::noise::{Basis, CellDistance, CellReturn};
use ve_world::terrain::{HeightGrid, MAX_SIDE, Noise, TerrainError, edge_falloff, generate};

use crate::{Fail, cache::Value, fail, fail_suggest};

/// The noise a tile's `basis` parameter picks; `None` (`classic`) is the original gradient noise, so existing worlds do not change.
fn terrain_basis(node: &Node) -> Option<Basis> {
    match node.params.get("basis") {
        Some(Param::Str(name)) => match name.as_str() {
            "perlin" => Some(Basis::Perlin),
            "simplex" => Some(Basis::Simplex),
            "value" => Some(Basis::Value),
            "cellular" => Some(Basis::Cellular {
                distance: CellDistance::Euclidean,
                ret: CellReturn::Distance,
                jitter: 1.0,
            }),
            _ => None,
        },
        _ => None,
    }
}

/// The most grid points along a tile side, margin included.
const SIDE: usize = MAX_SIDE + 1;
/// Cells of margin kept around a tile on every side (the reach of the occlusion mask is 32 cells).
pub(crate) const MARGIN: usize = 32;

/// How a tile's ground is generated, so any window of the same world can be generated again (for margins and for erosion).
#[derive(Clone, Copy)]
pub(crate) struct TerrainSource {
    pub(crate) noise: Noise,
    pub(crate) height: f32,
    /// Island radius and reach around the world origin, when the land falls to sea level.
    pub(crate) island: Option<(f32, f32)>,
    /// Terrace steps and sharpness.
    pub(crate) terraces: Option<(u32, f32)>,
}

impl TerrainSource {
    /// The ground over a window of `width` by `depth` cells starting at world `origin`.
    pub(crate) fn window(
        &self,
        origin: [f32; 2],
        cell: f32,
        width: usize,
        depth: usize,
        context: &ExecutionContext,
    ) -> Result<HeightGrid, Fail> {
        let mut grid = HeightGrid::new(width, depth, cell, origin).map_err(map_error)?;
        generate(&mut grid, &self.noise, self.height, &|| {
            context.is_cancelled()
        })
        .map_err(map_error)?;
        if let Some((radius, reach)) = self.island {
            edge_falloff(&mut grid, [0.0, 0.0], radius, reach);
        }
        if let Some((steps, sharpness)) = self.terraces {
            // Levels are fractions of the world height, not of this window's own range, so every tile cuts the same steps.
            #[expect(clippy::cast_precision_loss, reason = "the step count is at most 64")]
            let count = steps as f32;
            for h in grid.heights_mut() {
                let level =
                    (*h / self.height.max(f32::EPSILON) * count).round() / count * self.height;
                *h += (level - *h) * sharpness;
            }
        }
        Ok(grid)
    }
}

/// A tile: a grid with a margin, its source, and which part is the tile itself.
#[derive(Clone)]
pub(crate) struct Tile {
    /// Heights and layers over the tile plus `margin` cells on every side.
    pub(crate) grid: HeightGrid,
    pub(crate) margin: usize,
    pub(crate) source: TerrainSource,
}

impl Tile {
    /// Cells along x and z of the tile itself (without the margin).
    pub(crate) fn inner_cells(&self) -> (usize, usize) {
        (
            self.grid.width() - 2 * self.margin,
            self.grid.depth() - 2 * self.margin,
        )
    }

    /// The tile itself, margin cut off.
    pub(crate) fn inner(&self) -> Result<HeightGrid, Fail> {
        let (w, d) = self.inner_cells();
        self.grid
            .sub_grid(self.margin, self.margin, w, d)
            .map_err(map_error)
    }
}

pub(crate) fn run(node: &Node, context: &ExecutionContext) -> Option<Result<Value, Fail>> {
    (node.op.family().as_str() == "terrain.tile").then(|| build(node, context))
}

/// Triangles the node will make (two per cell), for the cost estimate.
pub(crate) fn estimate(node: &Node) -> usize {
    cells(node).map_or(0, |(x, z)| 2 * x * z)
}

pub(crate) fn param(node: &Node, name: &str) -> Result<f64, Fail> {
    node.params
        .get(name)
        .and_then(Param::as_f64)
        .ok_or_else(|| {
            fail(
                "param.type",
                format!(
                    "{} {name} is missing or malformed",
                    node.op.family().as_str()
                ),
            )
        })
}

fn size(node: &Node) -> Result<[f64; 2], Fail> {
    match node.params.get("size") {
        Some(Param::List(items)) if items.len() == 2 => {
            let (Some(x), Some(z)) = (items[0].as_f64(), items[1].as_f64()) else {
                return Err(fail("param.type", "terrain.tile size must be two numbers"));
            };
            Ok([x, z])
        }
        _ => Err(fail("param.type", "terrain.tile size must be two numbers")),
    }
}

/// Cells along x and z.
fn cells(node: &Node) -> Result<(usize, usize), Fail> {
    let (size, cell) = (size(node)?, param(node, "cell")?);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the result is range-checked right below"
    )]
    let count = |extent: f64| (extent / cell).round().max(1.0).min(1.0e6) as usize;
    let (x, z) = (count(size[0]), count(size[1]));
    if x + 2 * MARGIN + 1 > SIDE || z + 2 * MARGIN + 1 > SIDE {
        return Err(fail_suggest(
            "terrain.too_large",
            format!(
                "a tile of {x} by {z} cells (plus a margin of {MARGIN}) is over the limit of {} a side",
                SIDE - 1
            ),
            "raise `cell` or shrink `size`, and build more tiles with tile_x and tile_z",
        ));
    }
    Ok((x, z))
}

pub(crate) fn map_error(error: TerrainError) -> Fail {
    match error {
        TerrainError::Cancelled => fail("eval.cancelled", "terrain generation cancelled"),
        other => fail("terrain.failed", other.to_string()),
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "tile coordinates are bounded metres and small counts"
)]
fn build(node: &Node, context: &ExecutionContext) -> Result<Value, Fail> {
    let (cells_x, cells_z) = cells(node)?;
    let cell = param(node, "cell")? as f32;
    let (tile_x, tile_z) = (param(node, "tile_x")?, param(node, "tile_z")?);
    // The tile spans whole cells; neighbours start where it ends, so edges coincide.
    let span = [cells_x as f32 * cell, cells_z as f32 * cell];
    let origin = [tile_x as f32 * span[0], tile_z as f32 * span[1]];
    let radius = param(node, "island_radius")? as f32;
    let steps = param(node, "terraces")? as u32;
    let source = TerrainSource {
        noise: Noise {
            seed: param(node, "seed")? as u32,
            scale: param(node, "scale")? as f32,
            octaves: param(node, "octaves")? as u32,
            roughness: param(node, "roughness")? as f32,
            lacunarity: 2.0,
            ridged: matches!(node.params.get("style"), Some(Param::Str(name)) if name == "ridged"),
            warp: param(node, "warp")? as f32,
            basis: terrain_basis(node),
        },
        height: param(node, "height")? as f32,
        island: (radius > 0.0).then(|| (radius, param(node, "island_reach").unwrap_or(0.5) as f32)),
        terraces: (steps > 0).then(|| {
            (
                steps,
                param(node, "terrace_sharpness").unwrap_or(1.0) as f32,
            )
        }),
    };
    #[expect(clippy::cast_precision_loss, reason = "the margin is a small count")]
    let margin_metres = MARGIN as f32 * cell;
    let grid = source.window(
        [origin[0] - margin_metres, origin[1] - margin_metres],
        cell,
        cells_x + 1 + 2 * MARGIN,
        cells_z + 1 + 2 * MARGIN,
        context,
    )?;
    tile_value(Tile {
        grid,
        margin: MARGIN,
        source,
    })
}

/// The value a tile becomes: its inner quads as a polygon mesh with UVs, and the tile itself carried alongside.
pub(crate) fn tile_value(tile: Tile) -> Result<Value, Fail> {
    let inner = tile.inner()?;
    let surface = Surface::new(&inner);
    let failed = || {
        fail(
            "terrain.failed",
            "the height grid does not make a polygon mesh",
        )
    };
    let poly = surface.poly().ok_or_else(failed)?;
    // Two triangles per quad, (0, 1, 2) then (0, 2, 3), on the shared points: the same split the polygon mesh uses.
    let mut indices = Vec::with_capacity(surface.corners.len() / 4 * 6);
    let mut uvs = Vec::with_capacity(surface.corners.len() / 4 * 6);
    for (quad, ids) in surface.corners.chunks_exact(4).enumerate() {
        for k in [0, 1, 2, 0, 2, 3] {
            indices.push(ids[k]);
            uvs.push([surface.u[quad * 4 + k], surface.v[quad * 4 + k]]);
        }
    }
    let mut value = Value::solid(
        Mesh {
            positions: surface.positions,
            indices,
        },
        0.0,
    );
    value.uvs = uvs;
    value.poly = Some(Rc::new(poly));
    value.grid = Some(Rc::new(tile));
    Ok(value)
}

/// Unit vertex normals for a terrain tile's mesh, from the grid including its margin, so a tile's edge shades like its neighbour's.
/// Empty when the value is not a tile mesh any more (another node moved or rebuilt the surface).
pub(crate) fn vertex_normals(value: &Value) -> Vec<[f32; 3]> {
    let Some(tile) = value.grid.as_deref() else {
        return Vec::new();
    };
    let (w, d) = tile.inner_cells();
    let (points_x, points_z) = (w, d);
    if value.mesh.positions.len() != points_x * points_z {
        return Vec::new();
    }
    let grid = &tile.grid;
    let mut normals = Vec::with_capacity(points_x * points_z);
    for j in 0..points_z {
        for i in 0..points_x {
            let (x, z) = (grid.world_x(tile.margin + i), grid.world_z(tile.margin + j));
            normals.push(grid.normal(x, z));
        }
    }
    // The mesh must still be this tile's own surface: its first and last points sit where the grid says.
    let corner = |p: &[f32; 3], x: f32, z: f32| (p[0] - x).abs() < 1e-3 && (p[2] - z).abs() < 1e-3;
    let first = value
        .mesh
        .positions
        .first()
        .is_some_and(|p| corner(p, grid.world_x(tile.margin), grid.world_z(tile.margin)));
    let last = value.mesh.positions.last().is_some_and(|p| {
        corner(
            p,
            grid.world_x(tile.margin + points_x - 1),
            grid.world_z(tile.margin + points_z - 1),
        )
    });
    if first && last { normals } else { Vec::new() }
}

/// The grid's quads on its shared points, counter-clockwise seen from +y, with UVs across the tile per corner.
struct Surface {
    positions: Vec<[f32; 3]>,
    corners: Vec<u32>,
    u: Vec<f32>,
    v: Vec<f32>,
}

impl Surface {
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        reason = "grid indices are at most 2049 and fit u32"
    )]
    fn new(grid: &HeightGrid) -> Self {
        let (w, d) = (grid.width(), grid.depth());
        let mut positions = Vec::with_capacity(w * d);
        for j in 0..d {
            for i in 0..w {
                positions.push([grid.world_x(i), grid.heights()[j * w + i], grid.world_z(j)]);
            }
        }
        let at = |i: usize, j: usize| (j * w + i) as u32;
        let (mut corners, mut u, mut v) = (Vec::new(), Vec::new(), Vec::new());
        for j in 0..d - 1 {
            for i in 0..w - 1 {
                for (ci, cj) in [(i, j), (i, j + 1), (i + 1, j + 1), (i + 1, j)] {
                    corners.push(at(ci, cj));
                    u.push(ci as f32 / (w - 1) as f32);
                    v.push(cj as f32 / (d - 1) as f32);
                }
            }
        }
        Self {
            positions,
            corners,
            u,
            v,
        }
    }

    fn poly(&self) -> Option<PolyMesh> {
        let mut mesh = PolyMesh::new(
            self.positions.clone(),
            vec![4; self.corners.len() / 4],
            self.corners.clone(),
        )
        .ok()?;
        mesh.set_triangulation_mode(TriangulationMode::CurvedQuads);
        for (name, values) in [("u", self.u.clone()), ("v", self.v.clone())] {
            mesh.set_attribute(
                name,
                Attribute {
                    domain: Domain::Corner,
                    data: AttrData::F32(values),
                },
            )
            .ok()?;
        }
        Some(mesh)
    }
}
