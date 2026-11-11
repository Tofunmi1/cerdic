//! Edge subdivision.
//!
//! Every selected edge is cut `cuts` times, and the cut points are shared by the two faces on the edge. A quad with
//! two opposite cut edges becomes a strip of quads, a quad with all four edges cut becomes a grid (when `grid_fill`
//! is set), and any other face with cut edges is fanned from its centre. A nonzero `smooth` is left to the legacy build.

use std::collections::BTreeMap;

use crate::native::edit::{NO_ORIGIN, Soup};
use crate::{EditedMesh, KernelError, MeshOp, MeshSelection, PolygonMesh};

type V3 = [f64; 3];

fn point(s: &Soup, v: u32) -> V3 {
    s.positions[v as usize].map(f64::from)
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "the kernel stores f32 positions"
)]
fn to_f32(p: V3) -> [f32; 3] {
    [p[0] as f32, p[1] as f32, p[2] as f32]
}

fn lerp(a: V3, b: V3, t: f64) -> V3 {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Runs the operator natively, or returns `None` when it is not implemented yet.
#[must_use]
pub fn edit(
    mesh: &PolygonMesh<'_>,
    selection: &MeshSelection<'_>,
    op: &MeshOp,
) -> Option<Result<EditedMesh, KernelError>> {
    let MeshOp::Subdivide {
        cuts,
        smooth,
        grid_fill,
    } = *op
    else {
        return None;
    };
    Some(Soup::from_mesh(mesh).and_then(|soup| subdivide(soup, selection, cuts, smooth, grid_fill)))
}

fn subdivide(
    mut s: Soup,
    selection: &MeshSelection<'_>,
    cuts: u32,
    smooth: f64,
    grid_fill: bool,
) -> Result<EditedMesh, KernelError> {
    if !smooth.is_finite() {
        return Err(KernelError::NonFinite);
    }
    if !(1..=8).contains(&cuts) || !(0.0..=1.0).contains(&smooth) {
        return Err(KernelError::InvalidArgument("rejected by the kernel"));
    }
    if selection.edges.is_empty() {
        return Err(KernelError::InvalidArgument("rejected by the kernel"));
    }
    let count = cuts as usize;
    let known = s.edge_faces();
    // The cut points of each selected edge, ordered from its lower vertex to its higher one.
    let mut points: BTreeMap<(u32, u32), Vec<u32>> = BTreeMap::new();
    for &[a, b] in selection.edges {
        if a as usize >= s.positions.len() || b as usize >= s.positions.len() {
            return Err(KernelError::OutOfBounds);
        }
        let key = (a.min(b), a.max(b));
        if !known.contains_key(&key) {
            return Err(KernelError::InvalidArgument("rejected by the kernel"));
        }
        if points.contains_key(&key) {
            continue;
        }
        let (pa, pb) = (point(&s, key.0), point(&s, key.1));
        let mut ids = Vec::with_capacity(count);
        for i in 1..=count {
            #[expect(clippy::cast_precision_loss, reason = "at most 8 cuts")]
            let t = i as f64 / (count + 1) as f64;
            let push = s.arc_push(&known[&key], key.0, key.1, t, smooth);
            let on_edge = lerp(pa, pb, t);
            ids.push(s.add_vertex(
                to_f32([on_edge[0] + push[0], on_edge[1] + push[1], on_edge[2] + push[2]]),
                key.0,
            ));
        }
        points.insert(key, ids);
    }
    let old = s.faces.clone();
    let parents = s.face_parent.clone();
    let roles = s.face_role.clone();
    let mut faces: Vec<(Vec<u32>, u32, u32)> = Vec::new();
    for (fi, face) in old.iter().enumerate() {
        // The cut points along each edge, in the face's own direction.
        let along: Vec<Vec<u32>> = (0..face.len())
            .map(|i| {
                let (a, b) = (face[i], face[(i + 1) % face.len()]);
                match points.get(&(a.min(b), a.max(b))) {
                    Some(ids) if a < b => ids.clone(),
                    Some(ids) => ids.iter().rev().copied().collect(),
                    None => Vec::new(),
                }
            })
            .collect();
        let cut_edges: Vec<usize> = (0..face.len()).filter(|&i| !along[i].is_empty()).collect();
        if cut_edges.is_empty() {
            faces.push((face.clone(), parents[fi], roles[fi]));
            continue;
        }
        let parent = parents[fi];
        if face.len() == 4 && cut_edges.len() == 4 && grid_fill {
            grid(&mut s, face, &along, count, parent, &mut faces);
        } else if face.len() == 4 && cut_edges.len() == 2 && cut_edges[1] == cut_edges[0] + 2 {
            // Opposite edges: a strip. Rotate so the cut edges are the first and the third.
            let r = cut_edges[0];
            let q = [
                face[r],
                face[(r + 1) % 4],
                face[(r + 2) % 4],
                face[(r + 3) % 4],
            ];
            let (top, bottom_cuts) = (&along[r], &along[(r + 2) % 4]);
            let mut upper = vec![q[0]];
            upper.extend(top.iter().copied());
            upper.push(q[1]);
            // The third edge runs q2 -> q3: from q3 to q2 its points go in reverse.
            let mut lower = vec![q[3]];
            lower.extend(bottom_cuts.iter().rev().copied());
            lower.push(q[2]);
            for k in 0..=count {
                faces.push((
                    vec![upper[k], upper[k + 1], lower[k + 1], lower[k]],
                    parent,
                    roles[fi],
                ));
            }
        } else {
            // Any other face: its outline with the cut points in, fanned from the centre.
            let mut outline = Vec::new();
            for (i, &v) in face.iter().enumerate() {
                outline.push(v);
                outline.extend(along[i].iter().copied());
            }
            // Prefer a fan from one of the outline's own vertices (no new vertex): it must give every triangle area
            // facing the face's normal.
            let normal = s.face_normal(fi);
            let anchored = (0..outline.len()).find(|&anchor| {
                (0..outline.len()).all(|i| {
                    let j = (i + 1) % outline.len();
                    if i == anchor || j == anchor {
                        return true;
                    }
                    let (pa, pi, pj) = (
                        point(&s, outline[anchor]),
                        point(&s, outline[i]),
                        point(&s, outline[j]),
                    );
                    let (u, v) = (
                        [pi[0] - pa[0], pi[1] - pa[1], pi[2] - pa[2]],
                        [pj[0] - pa[0], pj[1] - pa[1], pj[2] - pa[2]],
                    );
                    let cross = [
                        u[1] * v[2] - u[2] * v[1],
                        u[2] * v[0] - u[0] * v[2],
                        u[0] * v[1] - u[1] * v[0],
                    ];
                    let along = cross[0] * normal[0] + cross[1] * normal[1] + cross[2] * normal[2];
                    let scale =
                        (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2])
                            .sqrt();
                    along > 1e-9 * scale
                })
            });
            if let Some(anchor) = anchored {
                for i in 0..outline.len() {
                    let j = (i + 1) % outline.len();
                    if i != anchor && j != anchor {
                        faces.push((
                            vec![outline[anchor], outline[i], outline[j]],
                            parent,
                            roles[fi],
                        ));
                    }
                }
                continue;
            }
            #[expect(
                clippy::cast_precision_loss,
                reason = "faces have a handful of corners"
            )]
            let centre = {
                let sum = outline.iter().fold([0.0; 3], |acc, &v| {
                    let p = point(&s, v);
                    [acc[0] + p[0], acc[1] + p[1], acc[2] + p[2]]
                });
                let n = outline.len() as f64;
                [sum[0] / n, sum[1] / n, sum[2] / n]
            };
            let hub = s.add_vertex(to_f32(centre), NO_ORIGIN);
            for i in 0..outline.len() {
                faces.push((
                    vec![outline[i], outline[(i + 1) % outline.len()], hub],
                    parent,
                    roles[fi],
                ));
            }
        }
    }
    s.faces = faces.iter().map(|f| f.0.clone()).collect();
    s.face_parent = faces.iter().map(|f| f.1).collect();
    s.face_role = faces.iter().map(|f| f.2).collect();
    Ok(s.finish())
}

/// A `(cuts + 1)` by `(cuts + 1)` grid of quads inside a quad whose four edges are all cut.
fn grid(
    s: &mut Soup,
    q: &[u32],
    along: &[Vec<u32>],
    count: usize,
    parent: u32,
    out: &mut Vec<(Vec<u32>, u32, u32)>,
) {
    let n = count + 1;
    // lattice[v][u]: u runs q0 -> q1 and v runs q0 -> q3.
    let mut lattice = vec![vec![0_u32; n + 1]; n + 1];
    for u in 0..=n {
        lattice[0][u] = match u {
            0 => q[0],
            x if x == n => q[1],
            x => along[0][x - 1],
        };
        lattice[n][u] = match u {
            0 => q[3],
            x if x == n => q[2],
            // The third edge runs q2 -> q3, so its points are reversed along q3 -> q2.
            x => along[2][count - x],
        };
    }
    for v in 0..=n {
        lattice[v][0] = match v {
            0 => q[0],
            x if x == n => q[3],
            // The fourth edge runs q3 -> q0: reversed along q0 -> q3.
            x => along[3][count - x],
        };
        lattice[v][n] = match v {
            0 => q[1],
            x if x == n => q[2],
            x => along[1][x - 1],
        };
    }
    let corners: Vec<V3> = q.iter().map(|&v| point(s, v)).collect();
    for v in 1..n {
        for u in 1..n {
            #[expect(clippy::cast_precision_loss, reason = "at most 9 lattice steps")]
            let (fu, fv) = (u as f64 / n as f64, v as f64 / n as f64);
            let top = lerp(corners[0], corners[1], fu);
            let bottom = lerp(corners[3], corners[2], fu);
            lattice[v][u] = s.add_vertex(to_f32(lerp(top, bottom, fv)), q[0]);
        }
    }
    for v in 0..n {
        for u in 0..n {
            out.push((
                vec![
                    lattice[v][u],
                    lattice[v][u + 1],
                    lattice[v + 1][u + 1],
                    lattice[v + 1][u],
                ],
                parent,
                0,
            ));
        }
    }
}
