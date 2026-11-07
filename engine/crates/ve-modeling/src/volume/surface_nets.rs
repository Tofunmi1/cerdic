//! `volume.to_mesh`: the zero level of a distance grid as a mesh of quads, by surface nets.
//!
//! Each grid cell the surface passes through gets one vertex, at the average of where the surface crosses that cell's edges; each grid edge the
//! surface crosses becomes one quad joining the four cells around it. The mesh is closed whenever the grid has outside all round its border
//! (a grid made by `volume.from_mesh` always does) and it faces outward. Almost every face is a quad; where the surface passes very close to a
//! grid corner (a crease left by a union or a subtract) a quad would be degenerate or twisted and is two triangles instead.

use std::rc::Rc;

use ve_geometry::PolyMesh;
use ve_graph::Node;
use ve_kernel::ExecutionContext;

use super::{input_volume, number};
use crate::poly_ops::value_from_poly;
use crate::{Fail, cache::Value, fail, fail_suggest};

/// The most quads one call makes.
const MAX_QUADS: usize = 500_000;

/// The twelve edges of a cell as pairs of corner numbers; corner bits are x 1, y 2, z 4.
const EDGES: [(usize, usize); 12] = [
    (0, 1),
    (2, 3),
    (4, 5),
    (6, 7),
    (0, 2),
    (1, 3),
    (4, 6),
    (5, 7),
    (0, 4),
    (1, 5),
    (2, 6),
    (3, 7),
];

pub(super) fn to_mesh(
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Result<Value, Fail> {
    let volume = input_volume(node, inputs, 0)?;
    let iso = number(node, "iso")? as f32;
    mesh_volume(node, &volume, iso, context)
}

/// The surface of `volume` at level `iso` as a polygon value (`node` names the operator in refusals).
pub(super) fn mesh_volume(
    node: &Node,
    volume: &super::Volume,
    iso: f32,
    context: &ExecutionContext,
) -> Result<Value, Fail> {
    let [nx, ny, nz] = volume.dims;
    if nx < 2 || ny < 2 || nz < 2 {
        return Err(fail("volume.empty", "the grid has no cells"));
    }
    let cells = [nx - 1, ny - 1, nz - 1];
    let cell_id = |i: usize, j: usize, k: usize| i + cells[0] * (j + cells[1] * k);
    let mut cell_vertex = vec![u32::MAX; cells[0] * cells[1] * cells[2]];
    let mut positions: Vec<[f32; 3]> = Vec::new();
    for k in 0..cells[2] {
        if k % 8 == 0 && context.is_cancelled() {
            return Err(fail("eval.cancelled", "meshing cancelled between slices"));
        }
        for j in 0..cells[1] {
            for i in 0..cells[0] {
                let mut corner = [0.0_f32; 8];
                let mut inside = 0;
                for (c, slot) in corner.iter_mut().enumerate() {
                    *slot = volume.values
                        [volume.index(i + (c & 1), j + ((c >> 1) & 1), k + ((c >> 2) & 1))]
                        - iso;
                    inside += usize::from(*slot < 0.0);
                }
                if inside == 0 || inside == 8 {
                    continue;
                }
                let mut sum = [0.0_f64; 3];
                let mut crossings = 0.0;
                for &(a, b) in &EDGES {
                    if (corner[a] < 0.0) == (corner[b] < 0.0) {
                        continue;
                    }
                    let t = f64::from(corner[a]) / f64::from(corner[a] - corner[b]);
                    for axis in 0..3 {
                        let (ca, cb) = (((a >> axis) & 1) as f64, ((b >> axis) & 1) as f64);
                        sum[axis] += ca + (cb - ca) * t;
                    }
                    crossings += 1.0;
                }
                let at = [i, j, k];
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "positions are stored as f32"
                )]
                let vertex = [0, 1, 2].map(|axis| {
                    (volume.origin[axis] + (at[axis] as f64 + sum[axis] / crossings) * volume.cell)
                        as f32
                });
                cell_vertex[cell_id(i, j, k)] = u32::try_from(positions.len()).unwrap_or(u32::MAX);
                positions.push(vertex);
            }
        }
    }
    // One quad per grid edge the surface crosses, from the four cells around the edge, wound to face away from the inside.
    let value = |i: usize, j: usize, k: usize| volume.values[volume.index(i, j, k)] - iso;
    let mut corners: Vec<u32> = Vec::new();
    let mut sizes: Vec<u32> = Vec::new();
    let mut push = |ring: [Option<(usize, usize, usize)>; 4], flip: bool| {
        let ids: Vec<u32> = ring
            .iter()
            .filter_map(|c| c.map(|(i, j, k)| cell_vertex[cell_id(i, j, k)]))
            .collect();
        if ids.len() != 4 || ids.contains(&u32::MAX) {
            return;
        }
        let ids = if flip {
            [ids[3], ids[2], ids[1], ids[0]]
        } else {
            [ids[0], ids[1], ids[2], ids[3]]
        };
        match split_for(&ids.map(|i| positions[i as usize])) {
            None => {
                corners.extend(ids);
                sizes.push(4);
            }
            Some(diagonal_02) => {
                // A degenerate or twisted quad (the surface passes close to a grid corner) is two triangles instead.
                let [a, b, c, d] = ids;
                let pair = if diagonal_02 {
                    [[a, b, c], [a, c, d]]
                } else {
                    [[b, c, d], [b, d, a]]
                };
                for t in pair {
                    corners.extend(t);
                    sizes.push(3);
                }
            }
        }
    };
    let ok = |i: isize, j: isize, k: isize| {
        (i >= 0
            && j >= 0
            && k >= 0
            && (i as usize) < cells[0]
            && (j as usize) < cells[1]
            && (k as usize) < cells[2])
            .then_some((i as usize, j as usize, k as usize))
    };
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let here = value(i, j, k);
                let (si, sj, sk) = (i as isize, j as isize, k as isize);
                if i + 1 < nx && (here < 0.0) != (value(i + 1, j, k) < 0.0) {
                    push(
                        [
                            ok(si, sj - 1, sk - 1),
                            ok(si, sj, sk - 1),
                            ok(si, sj, sk),
                            ok(si, sj - 1, sk),
                        ],
                        here >= 0.0,
                    );
                }
                if j + 1 < ny && (here < 0.0) != (value(i, j + 1, k) < 0.0) {
                    push(
                        [
                            ok(si - 1, sj, sk - 1),
                            ok(si - 1, sj, sk),
                            ok(si, sj, sk),
                            ok(si, sj, sk - 1),
                        ],
                        here >= 0.0,
                    );
                }
                if k + 1 < nz && (here < 0.0) != (value(i, j, k + 1) < 0.0) {
                    push(
                        [
                            ok(si - 1, sj - 1, sk),
                            ok(si, sj - 1, sk),
                            ok(si, sj, sk),
                            ok(si - 1, sj, sk),
                        ],
                        here >= 0.0,
                    );
                }
            }
        }
    }
    if corners.is_empty() {
        return Err(fail_suggest(
            "volume.no_surface",
            "the grid has no surface at this level: every voxel is on one side of it".to_owned(),
            "check iso (0 is the surface), or that an earlier step did not remove the whole shape",
        ));
    }
    let faces = sizes.len();
    if faces > MAX_QUADS {
        return Err(fail_suggest(
            "budget.volume",
            format!("the mesh would have {faces} faces (the most is {MAX_QUADS})"),
            "make the volume with a larger voxel_size",
        ));
    }
    let (kept, mut corners) = used_vertices(&mut positions, &mut corners);
    let build = |sizes: Vec<u32>, corners: Vec<u32>| -> Result<PolyMesh, Fail> {
        let mut mesh = PolyMesh::new(kept.clone(), sizes, corners).map_err(|error| {
            fail(
                "volume.mesh",
                format!(
                    "{}: the surface could not be made into a mesh: {error}",
                    node.op
                ),
            )
        })?;
        mesh.set_triangulation_mode(ve_geometry::TriangulationMode::CurvedQuads);
        Ok(mesh)
    };
    let mesh = build(sizes.clone(), corners.clone())?;
    match value_from_poly(node, mesh, None, context) {
        Ok(value) => Ok(value),
        Err(_) => {
            // The engine refused a quad that passed the checks above (one more way a flat quad can fold). Ask it about each quad on its own and
            // turn exactly the refused ones into triangles; triangles are always accepted.
            let mut demoted_sizes = Vec::with_capacity(sizes.len());
            let mut demoted_corners = Vec::with_capacity(corners.len());
            let mut at = 0;
            for &size in &sizes {
                let face = &corners[at..at + size as usize];
                at += size as usize;
                let fine = size != 4 || {
                    let points: Vec<[f32; 3]> = face.iter().map(|&c| kept[c as usize]).collect();
                    PolyMesh::new(points, vec![4], vec![0, 1, 2, 3]).is_ok_and(|mut one| {
                        one.set_triangulation_mode(ve_geometry::TriangulationMode::CurvedQuads);
                        one.triangulated_corner_indices(context).is_ok()
                    })
                };
                if fine {
                    demoted_sizes.push(size);
                    demoted_corners.extend_from_slice(face);
                } else {
                    demoted_sizes.extend([3, 3]);
                    demoted_corners.extend([face[0], face[1], face[2], face[0], face[2], face[3]]);
                }
            }
            corners = demoted_corners;
            value_from_poly(node, build(demoted_sizes, corners.clone())?, None, context)
        }
    }
}

/// `None` when the quad can stay a quad; otherwise `Some(true)` to split it along corners 0 and 2, `Some(false)` along 1 and 3.
///
/// A quad is kept when it has real area and both of its diagonals split it into two triangles that face the same way (a convex quad). The
/// engine refuses a polygon whose area is under a thousandth of its squared size or a flat bow-tie, so the area test here is ten times
/// stricter than that.
fn split_for(p: &[[f32; 3]; 4]) -> Option<bool> {
    let q = p.map(|v| v.map(f64::from));
    let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let cross = |a: [f64; 3], b: [f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let mut newell = [0.0_f64; 3];
    for i in 0..4 {
        let (a, b) = (q[i], q[(i + 1) % 4]);
        newell[0] += (a[1] - b[1]) * (a[2] + b[2]);
        newell[1] += (a[2] - b[2]) * (a[0] + b[0]);
        newell[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let mut low = q[0];
    let mut high = q[0];
    for v in &q {
        for axis in 0..3 {
            low[axis] = low[axis].min(v[axis]);
            high[axis] = high[axis].max(v[axis]);
        }
    }
    let extent_squared = dot(sub(high, low), sub(high, low));
    let magnitude = dot(newell, newell).sqrt();
    let along_02 = dot(
        cross(sub(q[1], q[0]), sub(q[2], q[0])),
        cross(sub(q[2], q[0]), sub(q[3], q[0])),
    );
    let along_13 = dot(
        cross(sub(q[2], q[1]), sub(q[3], q[1])),
        cross(sub(q[3], q[1]), sub(q[0], q[1])),
    );
    if magnitude > 0.02 * extent_squared && along_02 > 0.0 && along_13 > 0.0 {
        return None;
    }
    if along_02 > 0.0 {
        return Some(true);
    }
    if along_13 > 0.0 {
        return Some(false);
    }
    let (d02, d13) = (sub(q[2], q[0]), sub(q[3], q[1]));
    Some(dot(d02, d02) <= dot(d13, d13))
}

/// Keeps only the vertices some quad uses, renumbering the corners.
fn used_vertices(positions: &mut Vec<[f32; 3]>, corners: &mut [u32]) -> (Vec<[f32; 3]>, Vec<u32>) {
    let mut renumber = vec![u32::MAX; positions.len()];
    let mut kept = Vec::new();
    for c in corners.iter_mut() {
        let old = *c as usize;
        if renumber[old] == u32::MAX {
            renumber[old] = u32::try_from(kept.len()).unwrap_or(0);
            kept.push(positions[old]);
        }
        *c = renumber[old];
    }
    (kept, corners.to_vec())
}
