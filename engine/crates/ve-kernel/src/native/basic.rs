//! Easy mesh operators implemented without Blender.

use std::collections::{BTreeMap, BTreeSet};

use super::edit::{NO_ORIGIN, Soup};
use crate::{EditedMesh, KernelError, MeshOp, MeshSelection, PolygonMesh};

/// Runs an easy operator natively, returning `None` for operators handled by another owner.
#[must_use]
pub fn edit(
    mesh: &PolygonMesh<'_>,
    selection: &MeshSelection<'_>,
    op: &MeshOp,
) -> Option<Result<EditedMesh, KernelError>> {
    let result = match op {
        MeshOp::Triangulate => triangulate(mesh, selection),
        MeshOp::RemoveDoubles { distance } => remove_doubles(mesh, selection, *distance),
        MeshOp::RecalcNormals { inside } => recalc(mesh, *inside),
        MeshOp::SplitEdges => split_edges(mesh, selection),
        MeshOp::FillHoles { sides } => fill_holes(mesh, *sides),
        MeshOp::Symmetrize {
            axis,
            keep_positive,
            distance,
        } => symmetrize(mesh, *axis, *keep_positive, *distance),
        MeshOp::Mirror {
            point,
            normal,
            merge,
            distance,
        } => mirror(mesh, *point, *normal, *merge, *distance),
        MeshOp::Bisect {
            point,
            normal,
            distance,
            clear_inner,
            clear_outer,
            fill,
        } => bisect(
            mesh,
            *point,
            *normal,
            *distance,
            *clear_inner,
            *clear_outer,
            *fill,
        ),
        MeshOp::Dissolve { angle, boundaries } => dissolve(mesh, *angle, *boundaries),
        MeshOp::DissolveDegenerate { distance } => dissolve_degenerate(mesh, *distance),
        MeshOp::JoinTriangles {
            face_angle,
            shape_angle,
        } => join_triangles(mesh, *face_angle, *shape_angle),
        // The native draft rejects bent quads that the contract accepts: the legacy build serves it for now.
        MeshOp::PlanarFaces { .. } => return None,
        MeshOp::Smooth { factor, iterations } => smooth(mesh, *factor, *iterations),
        _ => return None,
    };
    // A cap outline this module cannot walk (pinched or branching) is left to the legacy build.
    if matches!(op, MeshOp::Bisect { .. }) && result == Err(KernelError::OperatorRefused) {
        return None;
    }
    Some(result)
}

fn soup(mesh: &PolygonMesh<'_>) -> Result<Soup, KernelError> {
    Soup::from_mesh(mesh)
}
fn sq(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]) * (a[i] - b[i])).sum()
}
fn xyz(p: [f32; 3]) -> [f64; 3] {
    p.map(f64::from)
}

#[expect(
    clippy::many_single_char_names,
    reason = "compact geometric predicates use conventional vertex labels"
)]
fn triangulate(
    mesh: &PolygonMesh<'_>,
    selection: &MeshSelection<'_>,
) -> Result<EditedMesh, KernelError> {
    let mut s = soup(mesh)?;
    let selected: BTreeSet<usize> = selection.faces.iter().map(|&x| x as usize).collect();
    if selected.iter().any(|&i| i >= s.faces.len()) {
        return Err(KernelError::OutOfBounds);
    }
    let mut faces = Vec::new();
    let mut parents = Vec::new();
    let mut roles = Vec::new();
    for (fi, face) in s.faces.iter().enumerate() {
        if face.len() == 3 || (!selected.is_empty() && !selected.contains(&fi)) {
            faces.push(face.clone());
            parents.push(s.face_parent[fi]);
            roles.push(s.face_role[fi]);
            continue;
        }
        let pts: Vec<[f64; 3]> = face.iter().map(|&v| xyz(s.positions[v as usize])).collect();
        let normal = s.face_normal(fi);
        let axis = (0..3)
            .max_by(|&a, &b| normal[a].abs().total_cmp(&normal[b].abs()))
            .unwrap_or(2);
        let uv: Vec<[f64; 2]> = pts
            .iter()
            .map(|p| match axis {
                0 => [p[1], p[2]],
                1 => [p[0], p[2]],
                _ => [p[0], p[1]],
            })
            .collect();
        let area = (0..face.len())
            .map(|i| {
                uv[i][0] * uv[(i + 1) % face.len()][1] - uv[(i + 1) % face.len()][0] * uv[i][1]
            })
            .sum::<f64>();
        let sign = if area >= 0.0 { 1.0 } else { -1.0 };
        let mut ring: Vec<usize> = (0..face.len()).collect();
        while ring.len() > 3 {
            let mut found = None;
            for k in 0..ring.len() {
                let (a, b, c) = (
                    ring[(k + ring.len() - 1) % ring.len()],
                    ring[k],
                    ring[(k + 1) % ring.len()],
                );
                let cross = (uv[b][0] - uv[a][0]) * (uv[c][1] - uv[b][1])
                    - (uv[b][1] - uv[a][1]) * (uv[c][0] - uv[b][0]);
                if sign * cross <= 1e-12 {
                    continue;
                }
                let inside = ring
                    .iter()
                    .copied()
                    .filter(|&q| q != a && q != b && q != c)
                    .any(|q| point_triangle(uv[q], uv[a], uv[b], uv[c], sign));
                if !inside {
                    found = Some((k, a, b, c));
                    break;
                }
            }
            let Some((k, a, b, c)) = found else {
                return Err(KernelError::OperatorRefused);
            };
            faces.push(vec![face[a], face[b], face[c]]);
            parents.push(s.face_parent[fi]);
            roles.push(0);
            ring.remove(k);
        }
        faces.push(ring.iter().map(|&i| face[i]).collect());
        parents.push(s.face_parent[fi]);
        roles.push(0);
    }
    s.faces = faces;
    s.face_parent = parents;
    s.face_role = roles;
    Ok(s.finish())
}
fn point_triangle(p: [f64; 2], a: [f64; 2], b: [f64; 2], c: [f64; 2], sign: f64) -> bool {
    let cross = |u: [f64; 2], v: [f64; 2], w: [f64; 2]| {
        (v[0] - u[0]) * (w[1] - v[1]) - (v[1] - u[1]) * (w[0] - v[0])
    };
    sign * cross(a, b, p) >= -1e-12
        && sign * cross(b, c, p) >= -1e-12
        && sign * cross(c, a, p) >= -1e-12
}

fn remove_doubles(
    mesh: &PolygonMesh<'_>,
    sel: &MeshSelection<'_>,
    distance: f64,
) -> Result<EditedMesh, KernelError> {
    if !distance.is_finite() || distance < 0.0 {
        return Err(KernelError::InvalidArgument(
            "distance must be non-negative",
        ));
    }
    let mut s = soup(mesh)?;
    let eligible: BTreeSet<usize> = if sel.vertices.is_empty() {
        (0..s.positions.len()).collect()
    } else {
        sel.vertices.iter().map(|&v| v as usize).collect()
    };
    if eligible.iter().any(|&v| v >= s.positions.len()) {
        return Err(KernelError::OutOfBounds);
    }
    let mut parent: Vec<usize> = (0..s.positions.len()).collect();
    for i in 0..s.positions.len() {
        if !eligible.contains(&i) {
            continue;
        }
        for j in 0..i {
            if eligible.contains(&j)
                && sq(xyz(s.positions[i]), xyz(s.positions[j])) <= distance * distance
            {
                let root = parent[j];
                parent[i] = root;
                break;
            }
        }
    }
    for f in &mut s.faces {
        for v in f.iter_mut() {
            *v = u32::try_from(parent[*v as usize]).map_err(|_| KernelError::TooLarge)?;
        }
        f.dedup();
        if f.len() > 1 && f[0] == *f.last().unwrap_or(&u32::MAX) {
            f.pop();
        }
    }
    let mut keep = Vec::new();
    let mut par = Vec::new();
    let mut role = Vec::new();
    for (i, f) in s.faces.iter().enumerate() {
        let uniq: BTreeSet<u32> = f.iter().copied().collect();
        if uniq.len() >= 3 {
            keep.push(f.clone());
            par.push(s.face_parent[i]);
            role.push(s.face_role[i]);
        }
    }
    s.faces = keep;
    s.face_parent = par;
    s.face_role = role;
    // Keep the earliest position and its provenance for each welded representative.
    let mut out = Soup::default();
    let mut remap = vec![u32::MAX; s.positions.len()];
    for f in &s.faces {
        for &old in f {
            let r = old as usize;
            if remap[r] == u32::MAX {
                remap[r] = out.add_vertex(s.positions[r], s.vertex_origin[r]);
            }
        }
    }
    for (i, f) in s.faces.iter().enumerate() {
        out.add_face(
            f.iter().map(|&v| remap[v as usize]).collect(),
            s.face_parent[i],
            s.face_role[i],
        );
    }
    Ok(out.finish())
}

#[expect(
    clippy::many_single_char_names,
    reason = "face and vertex indices use conventional mesh notation"
)]
fn recalc(mesh: &PolygonMesh<'_>, inside: bool) -> Result<EditedMesh, KernelError> {
    let mut s = soup(mesh)?;
    let edges = s.edge_faces();
    let mut adj = vec![Vec::<(usize, bool)>::new(); s.faces.len()];
    for uses in edges.values() {
        if uses.len() == 2 {
            let (a, ia) = uses[0];
            let (b, ib) = uses[1];
            let fa = &s.faces[a];
            let fb = &s.faces[b];
            let same = fa[ia] == fb[ib];
            adj[a].push((b, same));
            adj[b].push((a, same));
        }
    }
    let mut seen = vec![false; s.faces.len()];
    for start in 0..s.faces.len() {
        if seen[start] {
            continue;
        }
        seen[start] = true;
        let mut flip = vec![false; s.faces.len()];
        let mut stack = vec![start];
        let mut comp = Vec::new();
        while let Some(a) = stack.pop() {
            comp.push(a);
            for &(b, same) in &adj[a] {
                if !seen[b] {
                    flip[b] = flip[a] ^ same;
                    seen[b] = true;
                    stack.push(b);
                }
            }
        }
        for &fi in &comp {
            if flip[fi] {
                s.faces[fi].reverse();
            }
        }
        let volume: f64 = comp
            .iter()
            .map(|&fi| {
                let f = &s.faces[fi];
                let p = xyz(s.positions[f[0] as usize]);
                (1..f.len() - 1)
                    .map(|i| {
                        let a = p;
                        let b = xyz(s.positions[f[i] as usize]);
                        let c = xyz(s.positions[f[i + 1] as usize]);
                        a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                            + a[2] * (b[0] * c[1] - b[1] * c[0])
                    })
                    .sum::<f64>()
            })
            .sum();
        if (volume < 0.0) ^ inside {
            for fi in comp {
                s.faces[fi].reverse();
            }
        }
    }
    Ok(s.finish())
}

fn split_edges(mesh: &PolygonMesh<'_>, sel: &MeshSelection<'_>) -> Result<EditedMesh, KernelError> {
    let mut s = soup(mesh)?;
    let edges: BTreeSet<(u32, u32)> = sel
        .edges
        .iter()
        .map(|e| (e[0].min(e[1]), e[0].max(e[1])))
        .collect();
    let known = s.edge_faces();
    if edges.iter().any(|e| !known.contains_key(e)) {
        return Err(KernelError::InvalidArgument("selected edge is not in mesh"));
    }
    for edge in edges {
        if let Some(uses) = known.get(&edge) {
            for &(fi, _) in uses.iter().skip(1) {
                for v in [edge.0, edge.1] {
                    let p = s.positions[v as usize];
                    let o = s.vertex_origin[v as usize];
                    let nv = s.add_vertex(p, o);
                    for x in &mut s.faces[fi] {
                        if *x == v {
                            *x = nv;
                        }
                    }
                }
            }
        }
    }
    Ok(s.finish())
}

fn fill_holes(mesh: &PolygonMesh<'_>, sides: u32) -> Result<EditedMesh, KernelError> {
    let mut s = soup(mesh)?;
    let edges = s.edge_faces();
    let mut next = BTreeMap::<u32, u32>::new();
    for (_, uses) in edges {
        if uses.len() == 1 {
            let (fi, i) = uses[0];
            let f = &s.faces[fi];
            next.insert(f[(i + 1) % f.len()], f[i]);
        }
    }
    let mut visited = BTreeSet::new();
    let mut loops = Vec::new();
    for &start in next.keys() {
        if visited.contains(&start) {
            continue;
        }
        let mut lp = Vec::new();
        let mut cur = start;
        while visited.insert(cur) {
            lp.push(cur);
            let Some(&n) = next.get(&cur) else { break };
            cur = n;
            if cur == start {
                break;
            }
        }
        if cur == start && lp.len() >= 3 && lp.len() <= sides as usize {
            loops.push(lp);
        }
    }
    for lp in loops {
        s.add_face(lp, u32::MAX, 0);
    }
    Ok(s.finish())
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "the input plane distance is bounded by validated mesh coordinates"
)]
/// Cuts every face that crosses the plane `coordinate k = 0` and keeps the polygon on the wanted side (within `tolerance`
/// of the plane counts as on it). New vertices on the plane are shared between the faces of an edge.
fn clip_to_side(s: Soup, k: usize, positive: bool, tolerance: f64) -> Soup {
    let signed = |p: [f32; 3]| {
        if positive {
            f64::from(p[k])
        } else {
            -f64::from(p[k])
        }
    };
    let inside = |p: [f32; 3]| signed(p) >= -tolerance;
    let mut out = Soup {
        positions: s.positions.clone(),
        vertex_origin: s.vertex_origin.clone(),
        ..Soup::default()
    };
    let mut crossings: BTreeMap<(u32, u32), u32> = BTreeMap::new();
    for (fi, face) in s.faces.iter().enumerate() {
        let mut clipped: Vec<u32> = Vec::with_capacity(face.len() + 2);
        for i in 0..face.len() {
            let (a, b) = (face[i], face[(i + 1) % face.len()]);
            let (pa, pb) = (s.positions[a as usize], s.positions[b as usize]);
            let (ia, ib) = (inside(pa), inside(pb));
            if ia {
                clipped.push(a);
            }
            if ia != ib {
                let key = (a.min(b), a.max(b));
                let id = *crossings.entry(key).or_insert_with(|| {
                    let (sa, sb) = (signed(pa), signed(pb));
                    // Where the edge leaves the wanted side from a vertex that is already on the plane, that vertex is the crossing: a second vertex on
                    // top of it would leave zero-area slivers along the plane.
                    if ia && sa.abs() <= tolerance {
                        return a;
                    }
                    if ib && sb.abs() <= tolerance {
                        return b;
                    }
                    let t = sa / (sa - sb);
                    let mut p = [0.0_f32; 3];
                    for (d, slot) in p.iter_mut().enumerate() {
                        #[expect(
                            clippy::cast_possible_truncation,
                            reason = "positions are stored as f32"
                        )]
                        {
                            *slot = (f64::from(pa[d]) + (f64::from(pb[d]) - f64::from(pa[d])) * t)
                                as f32;
                        }
                    }
                    p[k] = 0.0;
                    let origin = if ia {
                        s.vertex_origin[a as usize]
                    } else {
                        s.vertex_origin[b as usize]
                    };
                    out.add_vertex(p, origin)
                });
                clipped.push(id);
            }
        }
        clipped.dedup();
        if clipped.first() == clipped.last() && clipped.len() > 1 {
            clipped.pop();
        }
        if clipped.len() >= 3 {
            out.add_face(clipped, s.face_parent[fi], s.face_role[fi]);
        }
    }
    out
}

fn symmetrize(
    mesh: &PolygonMesh<'_>,
    axis: u8,
    positive: bool,
    distance: f64,
) -> Result<EditedMesh, KernelError> {
    if axis > 2 {
        return Err(KernelError::InvalidArgument("axis must be 0, 1, or 2"));
    }
    let k = axis as usize;
    let s = clip_to_side(soup(mesh)?, k, positive, distance);
    let keep = |p: f32| {
        if positive {
            p >= -(distance as f32)
        } else {
            p <= distance as f32
        }
    };
    let original_count = s.positions.len();
    let mut remap = vec![u32::MAX; original_count];
    let mut out = Soup::default();
    for (i, mapped) in remap.iter_mut().enumerate().take(original_count) {
        let p = s.positions[i];
        if keep(p[k]) {
            let mut q = p;
            if q[k].abs() <= distance as f32 {
                q[k] = 0.0;
            }
            let origin = s.vertex_origin[i];
            *mapped = out.add_vertex(q, origin);
        }
    }
    for (fi, f) in s.faces.iter().enumerate() {
        if f.iter().all(|&v| remap[v as usize] != u32::MAX) {
            out.add_face(
                f.iter().map(|&v| remap[v as usize]).collect(),
                s.face_parent[fi],
                0,
            );
        }
    }
    let mut reflected = BTreeMap::new();
    for &v in remap.iter().filter(|&&v| v != u32::MAX) {
        let p = out.positions[v as usize];
        let nv = if p[k].abs() <= distance as f32 {
            v
        } else {
            let mut reflected_position = p;
            reflected_position[k] = -reflected_position[k];
            out.add_vertex(reflected_position, out.vertex_origin[v as usize])
        };
        reflected.insert(v, nv);
    }
    let originals = out.faces.clone();
    let parents = out.face_parent.clone();
    for (fi, f) in originals.iter().enumerate() {
        if f.iter()
            .any(|&v| out.positions[v as usize][k].abs() > distance as f32)
        {
            out.add_face(
                f.iter().rev().map(|v| reflected[v]).collect(),
                parents[fi],
                0,
            );
        }
    }
    Ok(out.finish())
}

#[expect(
    clippy::many_single_char_names,
    reason = "plane reflection uses conventional coordinate notation"
)]
#[expect(
    clippy::cast_possible_truncation,
    reason = "reflected coordinates are stored in the mesh's f32 position format"
)]
fn mirror(
    mesh: &PolygonMesh<'_>,
    point: [f64; 3],
    normal: [f64; 3],
    merge: bool,
    distance: f64,
) -> Result<EditedMesh, KernelError> {
    let len = normal.iter().map(|x| x * x).sum::<f64>().sqrt();
    if len < 1e-15 {
        return Err(KernelError::InvalidArgument("plane normal must be nonzero"));
    }
    let n = normal.map(|x| x / len);
    let mut s = soup(mesh)?;
    let count = s.positions.len();
    let mut remap = Vec::new();
    for i in 0..count {
        let p = xyz(s.positions[i]);
        let d = (0..3).map(|k| (p[k] - point[k]) * n[k]).sum::<f64>();
        let q = std::array::from_fn(|k| (p[k] - 2.0 * d * n[k]) as f32);
        if merge && d.abs() <= distance {
            remap.push(i as u32);
        } else {
            remap.push(s.add_vertex(q, s.vertex_origin[i]));
        }
    }
    let original = s.faces.clone();
    let parents = s.face_parent.clone();
    let roles = s.face_role.clone();
    for (fi, f) in original.iter().enumerate() {
        s.add_face(
            f.iter().rev().map(|&v| remap[v as usize]).collect(),
            parents[fi],
            roles[fi],
        );
    }
    let temp = s.finish();
    if merge {
        remove_doubles_from_result(temp, distance)
    } else {
        Ok(temp)
    }
}
#[expect(
    clippy::many_single_char_names,
    reason = "this adapter only forwards the mesh arrays"
)]
fn remove_doubles_from_result(m: EditedMesh, d: f64) -> Result<EditedMesh, KernelError> {
    let p = m.positions;
    let n = m.face_sizes;
    let c = m.corners;
    let mesh = PolygonMesh {
        positions: &p,
        face_sizes: &n,
        corners: &c,
    };
    remove_doubles(&mesh, &crate::MeshSelection::default(), d)
}

#[expect(
    clippy::many_single_char_names,
    reason = "polygon clipping uses conventional edge endpoint notation"
)]
#[expect(
    clippy::cast_possible_truncation,
    reason = "intersection coordinates are stored in the mesh's f32 position format"
)]
fn bisect(
    mesh: &PolygonMesh<'_>,
    point: [f64; 3],
    normal: [f64; 3],
    distance: f64,
    clear_inner: bool,
    clear_outer: bool,
    fill: bool,
) -> Result<EditedMesh, KernelError> {
    if clear_inner && clear_outer {
        return Err(KernelError::InvalidArgument(
            "choose at most one side to clear",
        ));
    }
    if !distance.is_finite() || distance < 0.0 {
        return Err(KernelError::InvalidArgument(
            "the plane distance must be finite and non-negative",
        ));
    }
    let len = normal.iter().map(|x| x * x).sum::<f64>().sqrt();
    if !(len > 1e-15) {
        return Err(KernelError::InvalidArgument("plane normal must be nonzero"));
    }
    let n = normal.map(|x| x / len);
    let s = soup(mesh)?;
    // Which side of the plane each vertex is on: 1 above (the normal side), -1 below, 0 on it.
    let dists: Vec<f64> = s
        .positions
        .iter()
        .map(|&p| {
            let p = xyz(p);
            (0..3).map(|k| (p[k] - point[k]) * n[k]).sum()
        })
        .collect();
    let class: Vec<i8> = dists
        .iter()
        .map(|&d| {
            if d > distance {
                1
            } else if d < -distance {
                -1
            } else {
                0
            }
        })
        .collect();
    // The side that survives: above when the inner side (opposite the normal) is cleared, below when the outer one is.
    let keep_side: Option<i8> = if clear_inner {
        Some(1)
    } else if clear_outer {
        Some(-1)
    } else {
        None
    };
    // `out` keeps the input vertices (unused ones are dropped at the end) and adds one vertex per crossed edge.
    let mut out = Soup {
        positions: s.positions.clone(),
        vertex_origin: s.vertex_origin.clone(),
        ..Soup::default()
    };
    let mut crossings = BTreeMap::<(u32, u32), u32>::new();
    let mut crossing = |out: &mut Soup, a: u32, b: u32| -> u32 {
        *crossings.entry((a.min(b), a.max(b))).or_insert_with(|| {
            let (da, db) = (dists[a as usize], dists[b as usize]);
            let t = da / (da - db);
            let (pa, pb) = (xyz(s.positions[a as usize]), xyz(s.positions[b as usize]));
            out.add_vertex(
                std::array::from_fn(|k| (pa[k] + t * (pb[k] - pa[k])) as f32),
                s.vertex_origin[a as usize],
            )
        })
    };
    for (fi, f) in s.faces.iter().enumerate() {
        let crosses = (0..f.len()).any(|i| {
            let (ca, cb) = (class[f[i] as usize], class[f[(i + 1) % f.len()] as usize]);
            ca * cb < 0
        });
        let sides: &[i8] = match keep_side {
            Some(1) => &[1],
            Some(_) => &[-1],
            None if crosses => &[1, -1],
            None => &[0],
        };
        for &side in sides {
            let mut poly = Vec::with_capacity(f.len() + 2);
            for i in 0..f.len() {
                let (a, b) = (f[i], f[(i + 1) % f.len()]);
                let keeps = side == 0 || class[a as usize] == 0 || class[a as usize] == side;
                if keeps {
                    poly.push(a);
                }
                if class[a as usize] * class[b as usize] < 0 {
                    poly.push(crossing(&mut out, a, b));
                }
            }
            poly.dedup();
            if poly.first() == poly.last() && poly.len() > 1 {
                poly.pop();
            }
            // A face lying in the plane belongs to neither side strictly: keep it only when it is the kept side.
            if poly.len() >= 3 {
                out.add_face(poly, s.face_parent[fi], s.face_role[fi]);
            }
        }
    }
    if fill && keep_side.is_some() {
        // The boundary of what is left that lies in the plane is the hole to cap. Walk its loops against the
        // direction the faces use so the cap faces outward.
        let on_plane = |v: u32| {
            let p = xyz(out.positions[v as usize]);
            let d: f64 = (0..3).map(|k| (p[k] - point[k]) * n[k]).sum();
            d.abs() <= distance.max(1e-6)
        };
        let mut next: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        let edges = out.edge_faces();
        for (&(lo, hi), users) in &edges {
            if users.len() != 1 || !on_plane(lo) || !on_plane(hi) {
                continue;
            }
            let (face, corner) = users[0];
            let (a, b) = (
                out.faces[face][corner],
                out.faces[face][(corner + 1) % out.faces[face].len()],
            );
            next.entry(b).or_default().push(a);
        }
        if next.values().any(|targets| targets.len() != 1) {
            // A pinched or branching outline: the legacy build decides.
            return Err(KernelError::OperatorRefused);
        }
        let mut loops: Vec<Vec<u32>> = Vec::new();
        let mut used = std::collections::BTreeSet::new();
        for &start in next.keys() {
            if used.contains(&start) {
                continue;
            }
            let mut cap = vec![start];
            used.insert(start);
            let mut here = start;
            while let Some(&to) = next.get(&here).and_then(|t| t.first()) {
                if to == start {
                    break;
                }
                if !used.insert(to) {
                    return Err(KernelError::OperatorRefused);
                }
                cap.push(to);
                here = to;
            }
            if cap.len() >= 3 {
                loops.push(cap);
            }
        }
        for cap in loops {
            out.add_face(cap, NO_ORIGIN, 0);
        }
    }
    Ok(out.finish())
}

#[expect(
    clippy::many_single_char_names,
    reason = "edge and face indices use conventional mesh notation"
)]
fn dissolve(
    mesh: &PolygonMesh<'_>,
    angle: f64,
    boundaries: bool,
) -> Result<EditedMesh, KernelError> {
    let mut s = soup(mesh)?;
    let mut changed = true;
    while changed {
        changed = false;
        let edges = s.edge_faces();
        for uses in edges.values() {
            if uses.len() == 1 && boundaries {
                // Boundary edges have no neighbouring face to merge; collapse the
                // collinear boundary vertex after the face merge pass below.
                continue;
            }
            if uses.len() != 2 {
                continue;
            }
            let (a, ia) = uses[0];
            let (b, _ib) = uses[1];
            let na = s.face_normal(a);
            let nb = s.face_normal(b);
            let cos = dot(na, nb) / (norm(na) * norm(nb)).max(1e-30);
            if cos.acos() > angle {
                continue;
            }
            let fa = s.faces[a].clone();
            let fb = s.faces[b].clone();
            let u = fa[ia];
            let v = fa[(ia + 1) % fa.len()];
            let Some(pos) = fb.iter().position(|&x| x == v) else {
                continue;
            };
            if fb[(pos + 1) % fb.len()] != u {
                continue;
            }
            let mut merged = fa[..=ia].to_vec();
            merged.extend(fb[(pos + 1)..].iter().copied());
            merged.extend(fb[..pos].iter().copied());
            merged.extend(fa[ia + 1..].iter().copied());
            merged.dedup();
            if merged.first() == merged.last() {
                merged.pop();
            }
            if merged.len() < 3 {
                continue;
            }
            let hi = a.max(b);
            let lo = a.min(b);
            s.faces[lo] = merged;
            s.faces.remove(hi);
            s.face_parent.remove(hi);
            s.face_role.remove(hi);
            changed = true;
            break;
        }
    }
    if boundaries {
        dissolve_collinear_vertices(&mut s, 1e-10);
    }
    Ok(s.finish())
}

#[expect(
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    clippy::cast_possible_truncation,
    reason = "compact vertex and edge predicates use conventional mesh notation and bounded u32 mesh indices"
)]
fn dissolve_collinear_vertices(s: &mut Soup, tolerance: f64) {
    let mut changed = true;
    while changed {
        changed = false;
        let edges = s.edge_faces();
        let mut adjacent = vec![BTreeSet::<u32>::new(); s.positions.len()];
        for &(a, b) in edges.keys() {
            adjacent[a as usize].insert(b);
            adjacent[b as usize].insert(a);
        }
        for v in 0..adjacent.len() {
            if adjacent[v].len() != 2 {
                continue;
            }
            let mut it = adjacent[v].iter();
            let (Some(&a), Some(&b)) = (it.next(), it.next()) else {
                continue;
            };
            let p = xyz(s.positions[v]);
            let x = xyz(s.positions[a as usize]);
            let y = xyz(s.positions[b as usize]);
            let u = [x[0] - p[0], x[1] - p[1], x[2] - p[2]];
            let w = [y[0] - p[0], y[1] - p[1], y[2] - p[2]];
            let cross = [
                u[1] * w[2] - u[2] * w[1],
                u[2] * w[0] - u[0] * w[2],
                u[0] * w[1] - u[1] * w[0],
            ];
            if norm(cross) > tolerance * (norm(u) * norm(w)).max(1.0) {
                continue;
            }
            let mut valid = true;
            for f in &mut s.faces {
                if let Some(i) = f.iter().position(|&q| q == v as u32) {
                    if f.len() <= 3 {
                        valid = false;
                        break;
                    }
                    f.remove(i);
                }
            }
            if valid {
                changed = true;
                break;
            }
        }
    }
}

fn dissolve_degenerate(mesh: &PolygonMesh<'_>, distance: f64) -> Result<EditedMesh, KernelError> {
    if !distance.is_finite() || distance < 0.0 {
        return Err(KernelError::InvalidArgument(
            "distance must be non-negative",
        ));
    }
    let s = soup(mesh)?;
    let threshold = distance * distance;
    let mut out = Soup::default();
    let mut map = vec![u32::MAX; s.positions.len()];
    for (fi, face) in s.faces.iter().enumerate() {
        let mut cleaned = Vec::new();
        for &v in face {
            if cleaned.last().is_some_and(|&last| {
                sq(
                    xyz(s.positions[v as usize]),
                    xyz(s.positions[last as usize]),
                ) <= threshold
            }) {
                continue;
            }
            cleaned.push(v);
        }
        if cleaned.len() > 1 && cleaned.first() == cleaned.last() {
            cleaned.pop();
        }
        if cleaned.len() < 3 {
            continue;
        }
        let ids = cleaned
            .into_iter()
            .map(|v| {
                let i = v as usize;
                if map[i] == u32::MAX {
                    map[i] = out.add_vertex(s.positions[i], s.vertex_origin[i]);
                }
                map[i]
            })
            .collect();
        out.add_face(ids, s.face_parent[fi], s.face_role[fi]);
    }
    Ok(out.finish())
}

#[expect(
    clippy::many_single_char_names,
    reason = "triangle edge notation is conventional and computed coordinates use the mesh's f32 position format"
)]
fn join_triangles(
    mesh: &PolygonMesh<'_>,
    face_angle: f64,
    shape_angle: f64,
) -> Result<EditedMesh, KernelError> {
    let mut s = soup(mesh)?;
    let mut changed = true;
    while changed {
        changed = false;
        for uses in s.edge_faces().values() {
            if uses.len() != 2 {
                continue;
            }
            let (a, ia) = uses[0];
            let (b, _) = uses[1];
            if s.faces[a].len() != 3 || s.faces[b].len() != 3 {
                continue;
            }
            let na = s.face_normal(a);
            let nb = s.face_normal(b);
            if (dot(na, nb) / (norm(na) * norm(nb)).max(1e-30))
                .clamp(-1.0, 1.0)
                .acos()
                > face_angle
            {
                continue;
            }
            let fa = s.faces[a].clone();
            let fb = s.faces[b].clone();
            let u = fa[ia];
            let v = fa[(ia + 1) % 3];
            let Some(j) = fb.iter().position(|&q| q == v) else {
                continue;
            };
            if fb[(j + 1) % 3] != u {
                continue;
            }
            // The first triangle walks u -> v -> w1 and the second v -> u -> w2: without the shared edge the
            // boundary is v -> w1 -> u -> w2.
            let (w1, w2) = (fa[(ia + 2) % 3], fb[(j + 2) % 3]);
            let q = vec![v, w1, u, w2];
            if w1 == w2 || q.iter().enumerate().any(|(i, x)| q[..i].contains(x)) {
                continue;
            }
            let pts: Vec<_> = q.iter().map(|&x| xyz(s.positions[x as usize])).collect();
            let mut shape_ok = true;
            for i in 0..4 {
                let p = pts[i];
                let x = pts[(i + 3) % 4];
                let y = pts[(i + 1) % 4];
                let a = [x[0] - p[0], x[1] - p[1], x[2] - p[2]];
                let b = [y[0] - p[0], y[1] - p[1], y[2] - p[2]];
                let angle = (dot(a, b) / (norm(a) * norm(b)).max(1e-30))
                    .clamp(-1.0, 1.0)
                    .acos();
                if (angle - std::f64::consts::FRAC_PI_2).abs() > shape_angle {
                    shape_ok = false;
                    break;
                }
            }
            if !shape_ok {
                continue;
            }
            // The merged quad must be convex (every corner turns the same way as the whole): a bent, dented or twisted
            // quad cannot be triangulated again later.
            let mut normal = [0.0_f64; 3];
            for i in 0..4 {
                let (p, r) = (pts[i], pts[(i + 1) % 4]);
                normal[0] += (p[1] - r[1]) * (p[2] + r[2]);
                normal[1] += (p[2] - r[2]) * (p[0] + r[0]);
                normal[2] += (p[0] - r[0]) * (p[1] + r[1]);
            }
            let scale = norm(normal);
            let convex = scale > 1e-20
                && (0..4).all(|i| {
                    let (p, m, r) = (pts[i], pts[(i + 1) % 4], pts[(i + 2) % 4]);
                    let (e1, e2) = (
                        [m[0] - p[0], m[1] - p[1], m[2] - p[2]],
                        [r[0] - m[0], r[1] - m[1], r[2] - m[2]],
                    );
                    let turn = [
                        e1[1] * e2[2] - e1[2] * e2[1],
                        e1[2] * e2[0] - e1[0] * e2[2],
                        e1[0] * e2[1] - e1[1] * e2[0],
                    ];
                    dot(turn, normal) > 1e-9 * scale * norm(e1) * norm(e2)
                });
            if !convex {
                continue;
            }
            let hi = a.max(b);
            let lo = a.min(b);
            s.faces[lo] = q;
            s.faces.remove(hi);
            s.face_parent.remove(hi);
            s.face_role.remove(hi);
            changed = true;
            break;
        }
    }
    Ok(s.finish())
}

#[expect(
    clippy::many_single_char_names,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "face-plane projection uses conventional mesh notation and mesh-sized counts are exactly represented for practical inputs"
)]
#[expect(
    dead_code,
    reason = "the draft waits until it accepts the bent quads the contract requires"
)]
fn planar_faces(
    mesh: &PolygonMesh<'_>,
    iterations: u32,
    factor: f64,
) -> Result<EditedMesh, KernelError> {
    let mut s = soup(mesh)?;
    for _ in 0..iterations.min(50) {
        for fi in 0..s.faces.len() {
            let f = s.faces[fi].clone();
            if f.len() < 4 {
                continue;
            }
            let n = s.face_normal(fi);
            let p0 = xyz(s.positions[f[0] as usize]);
            let center: [f64; 3] = std::array::from_fn(|k| {
                f.iter()
                    .map(|&v| f64::from(s.positions[v as usize][k]))
                    .sum::<f64>()
                    / f.len() as f64
            });
            for &v in &f {
                let p = xyz(s.positions[v as usize]);
                let d = dot([p[0] - p0[0], p[1] - p0[1], p[2] - p0[2]], n);
                for k in 0..3 {
                    let projected = p[k] - d * n[k];
                    let target = center[k] + projected - center[k];
                    s.positions[v as usize][k] = (p[k] * (1.0 - factor) + target * factor) as f32;
                }
            }
        }
    }
    Ok(s.finish())
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}
#[expect(
    clippy::needless_range_loop,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "coordinate-wise Laplacian updates use fixed three-axis arrays and mesh-sized valences"
)]
fn smooth(mesh: &PolygonMesh<'_>, factor: f64, iterations: u32) -> Result<EditedMesh, KernelError> {
    let mut s = soup(mesh)?;
    for _ in 0..iterations.min(50) {
        let mut adj = vec![BTreeSet::new(); s.positions.len()];
        for f in &s.faces {
            for i in 0..f.len() {
                let a = f[i] as usize;
                let b = f[(i + 1) % f.len()] as usize;
                adj[a].insert(b);
                adj[b].insert(a);
            }
        }
        let old = s.positions.clone();
        for (i, ns) in adj.iter().enumerate() {
            if ns.is_empty() {
                continue;
            }
            for k in 0..3 {
                let avg = ns.iter().map(|&j| f64::from(old[j][k])).sum::<f64>() / ns.len() as f64;
                s.positions[i][k] = (f64::from(old[i][k]) * (1.0 - factor) + avg * factor) as f32;
            }
        }
    }
    Ok(s.finish())
}
