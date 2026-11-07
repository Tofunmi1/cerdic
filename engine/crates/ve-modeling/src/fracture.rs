//! `poly.fracture`: one piece of a shape broken along a Voronoi pattern. The shape is sampled into a distance grid (as `volume.from_mesh` does), `pieces`
//! seed points are scattered inside it by a seeded generator, and each voxel belongs to the seed nearest it; the piece with number `piece` is the
//! shape's voxels that are nearer to its seed than to any other by at least half of `gap`, meshed by surface nets. Asked for every number from 0 to
//! `pieces - 1` (a loop writes the lines), the pieces tile the shape with cracks `gap` wide, and each is a closed solid of its own that a physics
//! body can use. Spike scope: random Voronoi cells only, no crack textures or interior faces.

use std::rc::Rc;

use ve_graph::{Node, Value as Param};
use ve_kernel::ExecutionContext;

use crate::cache::Value;
use crate::volume::{Volume, mesh_of, sample_mesh};
use crate::{Fail, fail, fail_suggest};

/// Most pieces one fracture makes.
pub const MAX_PIECES: usize = 64;

pub(crate) fn run(
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Option<Result<Value, Fail>> {
    (node.op.family().as_str() == "poly.fracture").then(|| fracture(node, inputs, context))
}

fn number(node: &Node, name: &str) -> Result<f64, Fail> {
    node.params
        .get(name)
        .and_then(Param::as_f64)
        .ok_or_else(|| fail("param.type", format!("{name} is missing or not a number")))
}

/// A seeded xorshift stream of numbers in [0, 1): the same seed gives the same seeds on every machine.
fn stream(seed: u64) -> impl FnMut() -> f64 {
    let mut state = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
    move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1_u64 << 53) as f64
    }
}

fn fracture(node: &Node, inputs: &[Rc<Value>], context: &ExecutionContext) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| fail("graph.missing_input", "poly.fracture needs a closed shape"))?;
    let pieces = number(node, "pieces")? as usize;
    let piece = number(node, "piece")? as usize;
    if pieces < 2 || pieces > MAX_PIECES {
        return Err(fail(
            "param.range",
            format!("poly.fracture makes 2 to {MAX_PIECES} pieces, got {pieces}"),
        ));
    }
    if piece >= pieces {
        return Err(fail_suggest(
            "param.range",
            format!(
                "piece {piece} does not exist: the pieces are numbered 0 to {}",
                pieces - 1
            ),
            "write a loop over piece = 0 to pieces - 1",
        ));
    }
    let voxel = number(node, "voxel_size")?;
    let gap = number(node, "gap")? as f32;
    let grid = sample_mesh(node, &input.mesh, voxel, 3)?;
    // Seeds: random voxel centres that are inside the shape, in a fixed order.
    let mut next = stream(number(node, "seed")? as u64);
    let [nx, ny, nz] = grid.dims;
    let mut seeds: Vec<[f64; 3]> = Vec::with_capacity(pieces);
    // Seeds keep a minimum distance from each other (two on one spot would leave both cells empty), a share of the spacing an even scatter would
    // have; if the shape is too thin for that, the distance is relaxed until they fit.
    let inside = grid.values.iter().filter(|v| **v < 0.0).count() as f64 * grid.cell.powi(3);
    let mut spacing = 0.7 * (inside / pieces as f64).cbrt();
    for _round in 0..6 {
        seeds.clear();
        for _try in 0..pieces * 400 {
            if seeds.len() == pieces {
                break;
            }
            let (i, j, k) = (
                (next() * nx as f64) as usize % nx,
                (next() * ny as f64) as usize % ny,
                (next() * nz as f64) as usize % nz,
            );
            if grid.values[grid.index(i, j, k)] >= -(voxel as f32) * 0.5 {
                continue;
            }
            let at = [
                grid.origin[0] + i as f64 * grid.cell,
                grid.origin[1] + j as f64 * grid.cell,
                grid.origin[2] + k as f64 * grid.cell,
            ];
            let apart = seeds.iter().all(|s| {
                ((at[0] - s[0]).powi(2) + (at[1] - s[1]).powi(2) + (at[2] - s[2]).powi(2)).sqrt()
                    >= spacing
            });
            if apart {
                seeds.push(at);
            }
        }
        if seeds.len() == pieces {
            break;
        }
        spacing *= 0.7;
    }
    if seeds.len() < pieces {
        return Err(fail_suggest(
            "fracture.thin",
            format!(
                "only {} of {pieces} seeds fit inside the shape at voxel_size {voxel}",
                seeds.len()
            ),
            "use a smaller voxel_size, fewer pieces, or a thicker shape",
        ));
    }
    let mut values = vec![0.0_f32; grid.values.len()];
    let trunc = grid.trunc;
    let mut any_inside = false;
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let at = grid.index(i, j, k);
                let p = [
                    grid.origin[0] + i as f64 * grid.cell,
                    grid.origin[1] + j as f64 * grid.cell,
                    grid.origin[2] + k as f64 * grid.cell,
                ];
                let (mut mine, mut other) = (f64::MAX, f64::MAX);
                for (n, s) in seeds.iter().enumerate() {
                    let d = ((p[0] - s[0]).powi(2) + (p[1] - s[1]).powi(2) + (p[2] - s[2]).powi(2))
                        .sqrt();
                    if n == piece {
                        mine = d;
                    } else {
                        other = other.min(d);
                    }
                }
                // Negative inside the piece: inside the shape, and nearer to this seed than to any other by half the gap.
                let bisector = ((mine - other) * 0.5) as f32 + gap * 0.5;
                let value = grid.values[at].max(bisector).clamp(-trunc, trunc);
                any_inside |= value < 0.0;
                values[at] = value;
            }
        }
    }
    if !any_inside {
        return Ok(Value::nothing());
    }
    let cell = Volume {
        origin: grid.origin,
        cell: grid.cell,
        dims: grid.dims,
        values,
        trunc,
    };
    mesh_of(node, &cell, 0.0, context).or_else(|error| {
        // A piece too small to hold a surface at this voxel size is nothing rather than an error.
        if error.code == "volume.no_surface" {
            Ok(Value::nothing())
        } else {
            Err(error)
        }
    })
}
