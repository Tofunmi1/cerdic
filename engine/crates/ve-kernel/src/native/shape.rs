//! Native shape mesh operators: extrude, inset, solidify and poke.
//!
//! Subdivide, wireframe, spin and loop cut still return `None`, so the legacy build serves them.

use std::collections::{BTreeMap, BTreeSet};

use crate::native::edit::{NO_ORIGIN, Soup};
use crate::{EditedMesh, KernelError, MeshOp, MeshSelection, PokeCenter, PolygonMesh};

type V3 = [f64; 3];

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn mul(a: V3, k: f64) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(a: V3) -> f64 {
    dot(a, a).sqrt()
}
fn unit(a: V3) -> V3 {
    let n = norm(a);
    if n > 1e-20 { mul(a, 1.0 / n) } else { [0.0; 3] }
}
fn point(s: &Soup, i: u32) -> V3 {
    s.positions[i as usize].map(f64::from)
}
#[expect(
    clippy::cast_possible_truncation,
    reason = "the kernel stores f32 positions"
)]
fn to_f32(v: V3) -> [f32; 3] {
    [v[0] as f32, v[1] as f32, v[2] as f32]
}
fn index(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(NO_ORIGIN)
}
fn selected(selection: &MeshSelection<'_>, count: usize) -> Vec<bool> {
    if selection.faces.is_empty() {
        return vec![true; count];
    }
    let mut flags = vec![false; count];
    for &f in selection.faces {
        if let Some(x) = flags.get_mut(f as usize) {
            *x = true;
        }
    }
    flags
}
fn require_selection(
    selection: &MeshSelection<'_>,
    count: usize,
) -> Result<Vec<bool>, KernelError> {
    if selection.faces.is_empty() {
        return Err(KernelError::InvalidArgument("rejected by the kernel"));
    }
    if selection.faces.iter().any(|&f| f as usize >= count) {
        return Err(KernelError::OutOfBounds);
    }
    Ok(selected(selection, count))
}

/// Selected faces are copied `offset` away and joined to what they leave behind by side walls.
fn extrude(
    mut s: Soup,
    selection: &MeshSelection<'_>,
    offset: V3,
) -> Result<EditedMesh, KernelError> {
    if offset.iter().any(|x| !x.is_finite()) {
        return Err(KernelError::NonFinite);
    }
    let flags = require_selection(selection, s.faces.len())?;
    let old_faces = s.faces.clone();
    let old_parents = s.face_parent.clone();
    let edge_faces = s.edge_faces();
    let mut duplicates: BTreeMap<u32, u32> = BTreeMap::new();
    for (fi, face) in old_faces.iter().enumerate() {
        if flags[fi] {
            for &v in face {
                if !duplicates.contains_key(&v) {
                    let moved = add(point(&s, v), offset);
                    let id = s.add_vertex(to_f32(moved), v);
                    duplicates.insert(v, id);
                }
            }
        }
    }
    let (mut faces, mut parents, mut roles) = (Vec::new(), Vec::new(), Vec::new());
    for (fi, face) in old_faces.iter().enumerate() {
        if !flags[fi] {
            faces.push(face.clone());
            parents.push(old_parents[fi]);
            roles.push(0);
            continue;
        }
        let copied: Vec<u32> = face.iter().map(|v| duplicates[v]).collect();
        faces.push(copied.clone());
        parents.push(old_parents[fi]);
        roles.push(1);
        for i in 0..face.len() {
            let (a, b) = (face[i], face[(i + 1) % face.len()]);
            let shared = edge_faces
                .get(&(a.min(b), a.max(b)))
                .is_some_and(|users| users.iter().any(|&(other, _)| other != fi && flags[other]));
            if !shared {
                faces.push(vec![a, b, copied[(i + 1) % face.len()], copied[i]]);
                parents.push(old_parents[fi]);
                roles.push(2);
            }
        }
    }
    s.faces = faces;
    s.face_parent = parents;
    s.face_role = roles;
    Ok(s.finish())
}

/// A second surface `thickness` away along the vertex normals, joined to the first at open edges.
fn solidify(mut s: Soup, thickness: f64) -> Result<EditedMesh, KernelError> {
    if !thickness.is_finite() {
        return Err(KernelError::NonFinite);
    }
    if thickness == 0.0 {
        return Err(KernelError::InvalidArgument("rejected by the kernel"));
    }
    let original = s.faces.clone();
    let edge_faces = s.edge_faces();
    if edge_faces.values().any(|users| users.len() > 2) {
        return Err(KernelError::OperatorRefused);
    }
    // Vertex normals: the face normals around the vertex, weighted by the angle each face has there.
    let mut vertex_normal: BTreeMap<u32, V3> = BTreeMap::new();
    for (fi, face) in original.iter().enumerate() {
        let n = unit(s.face_normal(fi));
        for i in 0..face.len() {
            let prev = point(&s, face[(i + face.len() - 1) % face.len()]);
            let here = point(&s, face[i]);
            let next = point(&s, face[(i + 1) % face.len()]);
            let (a, b) = (unit(sub(prev, here)), unit(sub(next, here)));
            let angle = dot(a, b).clamp(-1.0, 1.0).acos();
            let entry = vertex_normal.entry(face[i]).or_insert([0.0; 3]);
            *entry = add(*entry, mul(n, angle));
        }
    }
    // The flattest angle between a vertex normal and the faces around it: dividing the offset by its cosine keeps the
    // thickness true (measured across each face) where the surface bends, as at a box corner.
    let mut least_cosine: BTreeMap<u32, f64> = BTreeMap::new();
    for (fi, face) in original.iter().enumerate() {
        let n = unit(s.face_normal(fi));
        for &v in face {
            let c = dot(unit(vertex_normal[&v]), n);
            let entry = least_cosine.entry(v).or_insert(1.0);
            *entry = entry.min(c);
        }
    }
    let mut copies: BTreeMap<u32, u32> = BTreeMap::new();
    for face in &original {
        for &v in face {
            if copies.contains_key(&v) {
                continue;
            }
            let n = unit(vertex_normal[&v]);
            let stretch = 1.0 / least_cosine[&v].max(0.25);
            let moved = add(point(&s, v), mul(n, thickness * stretch));
            let id = s.add_vertex(to_f32(moved), v);
            copies.insert(v, id);
        }
    }
    // The solid lies between the two surfaces. Its outer skin faces away from the middle: with a positive thickness
    // the copy is the outer skin (keeps the winding) and the original turns inward; with a negative one the original
    // stays outward and the copy turns inward.
    let positive = thickness > 0.0;
    let (mut faces, mut parents, mut roles) = (Vec::new(), Vec::new(), Vec::new());
    for (fi, face) in original.iter().enumerate() {
        let mut kept = face.clone();
        if positive {
            kept.reverse();
        }
        faces.push(kept);
        parents.push(index(fi));
        roles.push(0);
    }
    for (fi, face) in original.iter().enumerate() {
        let mut shell: Vec<u32> = face.iter().map(|v| copies[v]).collect();
        if !positive {
            shell.reverse();
        }
        faces.push(shell);
        parents.push(index(fi));
        roles.push(1);
    }
    for (fi, face) in original.iter().enumerate() {
        for i in 0..face.len() {
            let (a, b) = (face[i], face[(i + 1) % face.len()]);
            let open = edge_faces
                .get(&(a.min(b), a.max(b)))
                .is_some_and(|users| users.len() == 1);
            if open {
                let wall = if positive {
                    vec![a, b, copies[&b], copies[&a]]
                } else {
                    vec![b, a, copies[&a], copies[&b]]
                };
                faces.push(wall);
                parents.push(index(fi));
                roles.push(2);
            }
        }
    }
    s.faces = faces;
    s.face_parent = parents;
    s.face_role = roles;
    Ok(s.finish())
}

struct InsetOptions {
    thickness: f64,
    depth: f64,
    even: bool,
    relative: bool,
    boundary: bool,
    outset: bool,
}

/// Shrinks the selected faces toward their inside, leaving a ring of faces along the region's border.
///
/// Selected faces that touch share their inner vertices, so a region of many faces is inset as one piece: only the
/// edges that leave the region get a ring face.
fn inset(
    mut s: Soup,
    selection: &MeshSelection<'_>,
    o: &InsetOptions,
) -> Result<EditedMesh, KernelError> {
    if !o.thickness.is_finite() || !o.depth.is_finite() {
        return Err(KernelError::NonFinite);
    }
    if o.thickness <= 0.0 {
        return Err(KernelError::InvalidArgument("rejected by the kernel"));
    }
    let flags = require_selection(selection, s.faces.len())?;
    let old = s.faces.clone();
    let old_parents = s.face_parent.clone();
    let edge_faces = s.edge_faces();
    let normals: Vec<V3> = (0..old.len()).map(|f| unit(s.face_normal(f))).collect();
    if (0..old.len()).any(|f| flags[f] && norm(normals[f]) < 0.5) {
        return Err(KernelError::InvalidArgument("rejected by the kernel"));
    }

    // Region edges: (a, b, face) for every edge of a selected face whose other side is not selected.
    struct Border {
        a: u32,
        b: u32,
        face: usize,
        fixed: bool,
    }
    let mut borders: Vec<Border> = Vec::new();
    for (fi, face) in old.iter().enumerate() {
        if !flags[fi] {
            continue;
        }
        for i in 0..face.len() {
            let (a, b) = (face[i], face[(i + 1) % face.len()]);
            let users = &edge_faces[&(a.min(b), a.max(b))];
            let across_selected = users.iter().any(|&(other, _)| other != fi && flags[other]);
            if across_selected {
                continue;
            }
            let on_mesh_boundary = users.len() == 1;
            borders.push(Border {
                a,
                b,
                face: fi,
                fixed: on_mesh_boundary && !o.boundary,
            });
        }
    }
    // Vertices of the region, and which border edges meet each.
    let mut region_vertices: BTreeSet<u32> = BTreeSet::new();
    for (fi, face) in old.iter().enumerate() {
        if flags[fi] {
            region_vertices.extend(face.iter().copied());
        }
    }
    let mut edges_at: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for (bi, border) in borders.iter().enumerate() {
        edges_at.entry(border.a).or_default().push(bi);
        edges_at.entry(border.b).or_default().push(bi);
    }
    let sign = if o.outset { -1.0 } else { 1.0 };
    let mut inner: BTreeMap<u32, u32> = BTreeMap::new();
    for &v in &region_vertices {
        let here = point(&s, v);
        let touching = edges_at.get(&v).map_or(&[][..], Vec::as_slice);
        // Selected faces' average normal here, for the depth.
        let mut lift = [0.0; 3];
        for (fi, face) in old.iter().enumerate() {
            if flags[fi] && face.contains(&v) {
                lift = add(lift, normals[fi]);
            }
        }
        let lift = mul(unit(lift), o.depth);
        if touching.iter().any(|&bi| borders[bi].fixed) {
            // A vertex on an edge that is not inset stays where it is, raised only by the depth.
            if norm(lift) < 1e-20 {
                inner.insert(v, v);
            } else {
                inner.insert(v, s.add_vertex(to_f32(add(here, lift)), v));
            }
            continue;
        }
        let mut direction_sum = [0.0; 3];
        let mut directions: Vec<V3> = Vec::new();
        let mut length_sum = 0.0;
        for &bi in touching {
            let border = &borders[bi];
            let (pa, pb) = (point(&s, border.a), point(&s, border.b));
            let along = unit(sub(pb, pa));
            let into = unit(cross(normals[border.face], along));
            directions.push(into);
            direction_sum = add(direction_sum, into);
            length_sum += norm(sub(pb, pa));
        }
        let mut thickness = o.thickness * sign;
        if o.relative && !touching.is_empty() {
            #[expect(
                clippy::cast_precision_loss,
                reason = "a vertex touches a handful of edges"
            )]
            let average = length_sum / touching.len() as f64;
            thickness *= average;
        }
        let shift = if directions.is_empty() {
            [0.0; 3]
        } else if o.even {
            // Miter: every border edge ends up the same distance from where it started.
            let mut best = mul(unit(direction_sum), thickness);
            if directions.len() == 2 {
                let c = dot(directions[0], directions[1]);
                best = mul(direction_sum, thickness / (1.0 + c).max(0.2));
            }
            best
        } else {
            mul(unit(direction_sum), thickness)
        };
        let moved = add(add(here, shift), lift);
        inner.insert(v, s.add_vertex(to_f32(moved), v));
    }
    let (mut faces, mut parents, mut roles) = (Vec::new(), Vec::new(), Vec::new());
    for (fi, face) in old.iter().enumerate() {
        if flags[fi] {
            faces.push(face.iter().map(|v| inner[v]).collect());
            parents.push(old_parents[fi]);
            roles.push(1);
        } else {
            faces.push(face.clone());
            parents.push(old_parents[fi]);
            roles.push(0);
        }
    }
    for border in &borders {
        if border.fixed {
            continue;
        }
        faces.push(vec![border.a, border.b, inner[&border.b], inner[&border.a]]);
        parents.push(old_parents[border.face]);
        roles.push(2);
    }
    s.faces = faces;
    s.face_parent = parents;
    s.face_role = roles;
    Ok(s.finish())
}

fn poke(
    mut s: Soup,
    selection: &MeshSelection<'_>,
    offset: f64,
    center: PokeCenter,
    relative: bool,
) -> Result<EditedMesh, KernelError> {
    let flags = selected(selection, s.faces.len());
    let old = s.faces.clone();
    let parents = s.face_parent.clone();
    let (mut faces, mut fp, mut roles) = (Vec::new(), Vec::new(), Vec::new());
    for (fi, f) in old.iter().enumerate() {
        if !flags[fi] {
            faces.push(f.clone());
            fp.push(parents[fi]);
            roles.push(0);
            continue;
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "faces have a handful of corners"
        )]
        let mean = mul(
            f.iter().fold([0.0; 3], |acc, &v| add(acc, point(&s, v))),
            1.0 / f.len() as f64,
        );
        let n = unit(s.face_normal(fi));
        let scale = if relative {
            norm(s.face_normal(fi)).sqrt()
        } else {
            1.0
        };
        let middle = match center {
            PokeCenter::Median | PokeCenter::Weighted => mean,
            PokeCenter::Bounds => {
                let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
                for &v in f {
                    let p = point(&s, v);
                    for k in 0..3 {
                        lo[k] = lo[k].min(p[k]);
                        hi[k] = hi[k].max(p[k]);
                    }
                }
                mul(add(lo, hi), 0.5)
            }
        };
        let apex = s.add_vertex(to_f32(add(middle, mul(n, offset * scale))), NO_ORIGIN);
        for i in 0..f.len() {
            faces.push(vec![f[i], f[(i + 1) % f.len()], apex]);
            fp.push(parents[fi]);
            roles.push(1);
        }
    }
    s.faces = faces;
    s.face_parent = fp;
    s.face_role = roles;
    Ok(s.finish())
}

/// Re-expresses `outer`, whose origins name elements of `inner`, in terms of what `inner`'s own origins name.
fn compose(mut outer: EditedMesh, inner: &EditedMesh) -> EditedMesh {
    let through = |origin: &mut u32, table: &[u32]| {
        if *origin != NO_ORIGIN {
            *origin = table.get(*origin as usize).copied().unwrap_or(NO_ORIGIN);
        }
    };
    for origin in &mut outer.vertex_origin {
        through(origin, &inner.vertex_origin);
    }
    for parent in &mut outer.face_parent {
        through(parent, &inner.face_parent);
    }
    outer
}

/// The input triangle each output triangle came from. The simplifier keeps the triangles it leaves alone as they were and rewrites a corner of one it
/// merges into a neighbour, so the input triangle that shares the most vertices with an output triangle is its parent.
fn simplified_parents(input: &[u32], output: &[u32], vertex_count: usize) -> Vec<u32> {
    let mut incident: Vec<Vec<u32>> = vec![Vec::new(); vertex_count];
    for (t, tri) in input.chunks_exact(3).enumerate() {
        for &v in tri {
            incident[v as usize].push(index(t));
        }
    }
    output
        .chunks_exact(3)
        .map(|tri| {
            let mut best = (0_usize, NO_ORIGIN);
            for &v in tri {
                for &t in &incident[v as usize] {
                    let from = &input[t as usize * 3..t as usize * 3 + 3];
                    let score = tri.iter().filter(|w| from.contains(w)).count();
                    if score > best.0 || (score == best.0 && t < best.1) {
                        best = (score, t);
                    }
                }
            }
            best.1
        })
        .collect()
}

/// Collapses edges of a triangle mesh until about `ratio` of the triangles remain, keeping each survivor's parent and each vertex's origin.
/// With `lock_border` the vertices on open edges stay put.
fn simplify_triangles(
    triangles: &EditedMesh,
    ratio: f64,
    lock_border: bool,
) -> Result<EditedMesh, KernelError> {
    let simple = crate::simplify_with(
        &crate::Mesh {
            positions: triangles.positions.clone(),
            indices: triangles.corners.clone(),
        },
        ratio,
        1.0,
        lock_border,
    )?;
    let parents = simplified_parents(&triangles.corners, &simple.indices, triangles.positions.len());
    let mut soup = Soup {
        positions: triangles.positions.clone(),
        vertex_origin: triangles.vertex_origin.clone(),
        ..Soup::default()
    };
    for (t, corners) in simple.indices.chunks_exact(3).enumerate() {
        let parent = match parents[t] {
            NO_ORIGIN => NO_ORIGIN,
            from => triangles.face_parent[from as usize],
        };
        soup.add_face(corners.to_vec(), parent, 0);
    }
    Ok(soup.finish())
}

/// `mesh` with every face a triangle.
fn triangulated(mesh: EditedMesh) -> Result<EditedMesh, KernelError> {
    if mesh.face_sizes.iter().all(|&n| n == 3) {
        return Ok(mesh);
    }
    let cut = crate::native::basic::edit(&mesh.as_mesh(), &MeshSelection::default(), &MeshOp::Triangulate)
        .ok_or(KernelError::OperatorRefused)??;
    Ok(compose(cut, &mesh))
}

/// `mesh` without the faces `drop` picks, origins unchanged.
fn without_faces(mesh: &EditedMesh, drop: impl Fn(usize) -> bool) -> Result<EditedMesh, KernelError> {
    let source = Soup::from_mesh(&mesh.as_mesh())?;
    let mut kept = Soup {
        positions: source.positions.clone(),
        vertex_origin: source.vertex_origin.clone(),
        ..Soup::default()
    };
    for (f, face) in source.faces.iter().enumerate() {
        if !drop(f) {
            kept.add_face(face.clone(), index(f), 0);
        }
    }
    Ok(compose(kept.finish(), mesh))
}

/// The faces of `mesh` that lie on the positive side of the plane through the origin, plane faces included, with origins naming `mesh`'s own elements.
fn positive_half(mesh: &EditedMesh, axis: usize, tolerance: f32) -> Result<EditedMesh, KernelError> {
    let source = Soup::from_mesh(&mesh.as_mesh())?;
    let mut half = Soup {
        positions: source.positions.clone(),
        vertex_origin: source.vertex_origin.clone(),
        ..Soup::default()
    };
    for (f, face) in source.faces.iter().enumerate() {
        if face.iter().all(|&v| source.positions[v as usize][axis] >= -tolerance) {
            half.add_face(face.clone(), index(f), 0);
        }
    }
    Ok(half.finish())
}

/// Decimates so the result is exactly symmetric about the plane through the origin perpendicular to `axis`: the mesh is cut at the plane and mirrored from its
/// positive side, that half is decimated with the vertices on the cut held fixed, and the result is mirrored back. A mesh that is not already symmetric
/// comes out as the mirror of its positive side.
fn decimate_symmetric(
    triangles: EditedMesh,
    ratio: f64,
    axis: u8,
) -> Result<EditedMesh, KernelError> {
    if axis > 2 {
        return Err(KernelError::InvalidArgument("axis must be 0, 1, or 2"));
    }
    let k = axis as usize;
    let (low, high) = triangles
        .positions
        .iter()
        .fold((f32::MAX, f32::MIN), |(low, high), p| (low.min(p[k]), high.max(p[k])));
    let extent = triangles
        .positions
        .iter()
        .flat_map(|p| p.iter().copied())
        .fold(0.0_f32, |m, c| m.max(c.abs()))
        .max(high - low);
    let tolerance = extent * 1e-4;
    let mirror = MeshOp::Symmetrize { axis, keep_positive: true, distance: f64::from(tolerance) };
    let symmetric = |mesh: &EditedMesh| {
        crate::native::basic::edit(&mesh.as_mesh(), &MeshSelection::default(), &mirror)
            .ok_or(KernelError::OperatorRefused)?
            .map(|made| compose(made, mesh))
    };
    let whole = symmetric(&triangles)?;
    let half = compose(positive_half(&whole, k, tolerance)?, &whole);
    if half.face_sizes.is_empty() {
        return Err(KernelError::InvalidArgument("rejected by the kernel"));
    }
    let half = triangulated(half)?;
    // The simplifier passes `half`'s origins straight through, and `half` already names input elements.
    let small = simplify_triangles(&half, ratio, true)?;
    // With the cut held fixed, collapsing the last interior vertex beside it can leave a triangle whose three corners all lie on the plane. It is not mirrored,
    // so its edges would join three faces: it goes, unless the input half already had it.
    let key = |mesh: &EditedMesh, face: usize| -> Option<[[u32; 3]; 3]> {
        let corners = &mesh.corners[face * 3..face * 3 + 3];
        let mut at: Vec<[u32; 3]> = corners.iter().map(|&v| mesh.positions[v as usize].map(f32::to_bits)).collect();
        at.sort_unstable();
        corners
            .iter()
            .all(|&v| mesh.positions[v as usize][k].abs() <= tolerance)
            .then(|| [at[0], at[1], at[2]])
    };
    let present: BTreeSet<[[u32; 3]; 3]> = (0..half.face_sizes.len()).filter_map(|f| key(&half, f)).collect();
    let small = without_faces(&small, |f| key(&small, f).is_some_and(|flat| !present.contains(&flat)))?;
    let mirrored = crate::native::basic::edit(&small.as_mesh(), &MeshSelection::default(), &mirror)
        .ok_or(KernelError::OperatorRefused)??;
    Ok(compose(mirrored, &small))
}

/// Collapses edges until about `ratio` of the faces remain, with the vendored meshoptimizer. With `symmetry` the result is symmetric about that axis.
fn decimate(
    mesh: &PolygonMesh<'_>,
    ratio: f64,
    triangulate: bool,
    symmetry: Option<u8>,
) -> Result<EditedMesh, KernelError> {
    if !ratio.is_finite() {
        return Err(KernelError::NonFinite);
    }
    if !(ratio > 0.0 && ratio <= 1.0) {
        return Err(KernelError::InvalidArgument("rejected by the kernel"));
    }
    let all_triangles = mesh.face_sizes.iter().all(|&n| n == 3);
    let triangles = if all_triangles {
        Soup::from_mesh(mesh)?.finish()
    } else if triangulate {
        crate::native::basic::edit(mesh, &MeshSelection::default(), &MeshOp::Triangulate)
            .ok_or(KernelError::OperatorRefused)??
    } else {
        return Err(KernelError::InvalidArgument("rejected by the kernel"));
    };
    match symmetry {
        Some(axis) => decimate_symmetric(triangles, ratio, axis),
        None => simplify_triangles(&triangles, ratio, false),
    }
}

/// Runs the native implementation, or returns `None` when this module does not implement the operator yet.
#[must_use]
pub fn edit(
    mesh: &PolygonMesh<'_>,
    selection: &MeshSelection<'_>,
    op: &MeshOp,
) -> Option<Result<EditedMesh, KernelError>> {
    if let MeshOp::Decimate {
        ratio,
        symmetry,
        triangulate,
    } = *op
    {
        return Some(decimate(mesh, ratio, triangulate, symmetry));
    }
    if !matches!(
        op,
        MeshOp::Extrude { .. }
            | MeshOp::Solidify { .. }
            | MeshOp::Inset { .. }
            | MeshOp::Poke { .. }
    ) {
        return None;
    }
    let result = Soup::from_mesh(mesh).and_then(|s| match *op {
        MeshOp::Extrude { offset } => extrude(s, selection, offset),
        MeshOp::Solidify { thickness } => solidify(s, thickness),
        MeshOp::Inset {
            thickness,
            depth,
            even,
            relative,
            boundary,
            outset,
        } => inset(
            s,
            selection,
            &InsetOptions {
                thickness,
                depth,
                even,
                relative,
                boundary,
                outset,
            },
        ),
        MeshOp::Poke {
            offset,
            center,
            relative,
        } => poke(s, selection, offset, center, relative),
        _ => Err(KernelError::InvalidArgument(
            "unsupported native shape operator",
        )),
    });
    Some(result)
}
