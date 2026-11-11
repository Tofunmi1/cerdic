//! Native wireframe, unsubdivide, rotate edges and connect vertices operators.

use crate::native::edit::Soup;
use crate::{EditedMesh, KernelError, MeshOp, MeshSelection, PolygonMesh};

/// Runs the operator natively, or returns `None` when it is not implemented yet.
#[must_use]
pub fn edit(
    mesh: &PolygonMesh<'_>,
    selection: &MeshSelection<'_>,
    op: &MeshOp,
) -> Option<Result<EditedMesh, KernelError>> {
    match op {
        MeshOp::Wireframe {
            thickness,
            offset,
            even,
            relative,
            boundary,
            replace,
        } => Some(wireframe(
            mesh, selection, *thickness, *offset, *even, *relative, *boundary, *replace,
        )),
        MeshOp::Unsubdivide { .. } => None,
        MeshOp::RotateEdges { counter_clockwise } => {
            Some(rotate_edges(mesh, selection, *counter_clockwise))
        }
        MeshOp::ConnectVertices { check_degenerate } => {
            Some(connect_vertices(mesh, selection, *check_degenerate))
        }
        MeshOp::Beautify { .. } => None,
        _ => None,
    }
}

fn connect_vertices(
    mesh: &PolygonMesh<'_>,
    selection: &MeshSelection<'_>,
    check_degenerate: bool,
) -> Result<EditedMesh, KernelError> {
    let s = Soup::from_mesh(mesh)?;
    if selection
        .vertices
        .iter()
        .any(|&v| v as usize >= s.positions.len())
    {
        return Err(KernelError::OutOfBounds);
    }
    let selected: std::collections::BTreeSet<u32> = selection.vertices.iter().copied().collect();
    let faces = s.faces.clone();
    let parents = s.face_parent.clone();
    let roles = s.face_role.clone();
    let mut out = Soup {
        positions: s.positions.clone(),
        vertex_origin: s.vertex_origin.clone(),
        ..Soup::default()
    };
    for (fi, face) in faces.iter().enumerate() {
        let hits: Vec<usize> = face
            .iter()
            .enumerate()
            .filter_map(|(i, v)| selected.contains(v).then_some(i))
            .collect();
        if hits.len() != 2 || hits[0] + 1 == hits[1] || (hits[0] == 0 && hits[1] + 1 == face.len())
        {
            out.add_face(face.clone(), parents[fi], roles[fi]);
            continue;
        }
        let (i, j) = (hits[0], hits[1]);
        if check_degenerate && (j - i < 2 || face.len() - (j - i) < 2) {
            out.add_face(face.clone(), parents[fi], roles[fi]);
            continue;
        }
        let first = face[i..=j].to_vec();
        let mut second = face[j..].to_vec();
        second.extend_from_slice(&face[..=i]);
        if first.len() >= 3 && second.len() >= 3 {
            out.add_face(first, parents[fi], 0);
            out.add_face(second, parents[fi], 0);
        } else {
            out.add_face(face.clone(), parents[fi], roles[fi]);
        }
    }
    Ok(out.finish())
}

fn rotate_edges(
    mesh: &PolygonMesh<'_>,
    selection: &MeshSelection<'_>,
    counter_clockwise: bool,
) -> Result<EditedMesh, KernelError> {
    let mut s = Soup::from_mesh(mesh)?;
    let edges = s.edge_faces();
    let requested: Vec<(u32, u32)> = if selection.edges.is_empty() {
        Vec::new()
    } else {
        let mut list = Vec::new();
        for &[a, b] in selection.edges {
            let key = (a.min(b), a.max(b));
            if !edges.contains_key(&key) {
                return Err(KernelError::OutOfBounds);
            }
            list.push(key);
        }
        list
    };
    for key in requested {
        let Some(users) = edges.get(&key) else {
            continue;
        };
        if users.len() != 2 {
            continue;
        }
        let (fa, ia) = users[0];
        let (fb, ib) = users[1];
        if s.faces[fa].len() != 3 || s.faces[fb].len() != 3 {
            continue;
        }
        let (u, v) = (s.faces[fa][ia], s.faces[fa][(ia + 1) % 3]);
        if s.faces[fb][ib] != v || s.faces[fb][(ib + 1) % 3] != u {
            continue;
        }
        let (x, y) = (s.faces[fa][(ia + 2) % 3], s.faces[fb][(ib + 2) % 3]);
        if x == y || edges.contains_key(&(x.min(y), x.max(y))) {
            continue;
        }
        let (left, right) = if counter_clockwise {
            ((x, y, u), (x, v, y))
        } else {
            ((x, u, y), (x, y, v))
        };
        s.faces[fa] = vec![left.0, left.1, left.2];
        s.faces[fb] = vec![right.0, right.1, right.2];
    }
    Ok(s.finish())
}

fn wireframe(
    mesh: &PolygonMesh<'_>,
    selection: &MeshSelection<'_>,
    thickness: f64,
    offset: f64,
    even: bool,
    relative: bool,
    boundary: bool,
    replace: bool,
) -> Result<EditedMesh, KernelError> {
    if !thickness.is_finite() || !offset.is_finite() {
        return Err(KernelError::NonFinite);
    }
    if thickness <= 0.0 {
        return Err(KernelError::InvalidArgument("thickness must be positive"));
    }
    let s = Soup::from_mesh(mesh)?;
    // With `boundary` off, the struts along the mesh's open edges keep no outer side wall.
    let open_edges: std::collections::BTreeSet<(u32, u32)> = if boundary {
        std::collections::BTreeSet::new()
    } else {
        s.edge_faces()
            .into_iter()
            .filter(|(_, users)| users.len() == 1)
            .map(|(edge, _)| edge)
            .collect()
    };
    if selection.faces.iter().any(|&f| f as usize >= s.faces.len()) {
        return Err(KernelError::OutOfBounds);
    }
    let selected: Vec<bool> = if selection.faces.is_empty() {
        vec![true; s.faces.len()]
    } else {
        let mut flags = vec![false; s.faces.len()];
        for &face in selection.faces {
            flags[face as usize] = true;
        }
        flags
    };
    let original = s.faces.clone();
    let parents = s.face_parent.clone();
    let roles = s.face_role.clone();
    let mut output = Soup {
        positions: s.positions.clone(),
        vertex_origin: s.vertex_origin.clone(),
        ..Soup::default()
    };
    for (fi, face) in original.iter().enumerate() {
        if !selected[fi] || !replace {
            output.add_face(face.clone(), parents[fi], roles[fi]);
        }
        if !selected[fi] {
            continue;
        }
        let center: [f64; 3] = std::array::from_fn(|k| {
            face.iter()
                .map(|&v| f64::from(s.positions[v as usize][k]))
                .sum::<f64>()
                / face.len() as f64
        });
        let normal = s.face_normal(fi);
        let length = (normal.iter().map(|x| x * x).sum::<f64>()).sqrt();
        if length <= 1e-20 {
            return Err(KernelError::InvalidArgument("degenerate face"));
        }
        let normal = normal.map(|x| x / length);
        let average = face
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                let w = face[(i + 1) % face.len()];
                let p = s.positions[v as usize].map(f64::from);
                let q = s.positions[w as usize].map(f64::from);
                ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
            })
            .sum::<f64>()
            / face.len() as f64;
        let width = thickness * if relative { average } else { 1.0 };
        let _ = even;
        let mut outer = Vec::with_capacity(face.len());
        let mut inner = Vec::with_capacity(face.len());
        let mut back_outer = Vec::with_capacity(face.len());
        let mut back_inner = Vec::with_capacity(face.len());
        for &v in face {
            let p = s.positions[v as usize];
            let out: [f64; 3] = std::array::from_fn(|k| f64::from(p[k]) + normal[k] * offset);
            let inside: [f64; 3] = std::array::from_fn(|k| {
                center[k]
                    + (f64::from(p[k]) - center[k]) * (1.0 - 2.0 * width / average.max(1e-20))
                    + normal[k] * offset
            });
            let back_out: [f64; 3] = std::array::from_fn(|k| out[k] - normal[k] * width);
            let back_in: [f64; 3] = std::array::from_fn(|k| inside[k] - normal[k] * width);
            outer.push(output.add_vertex(out.map(|x| x as f32), v));
            inner.push(output.add_vertex(inside.map(|x| x as f32), v));
            back_outer.push(output.add_vertex(back_out.map(|x| x as f32), v));
            back_inner.push(output.add_vertex(back_in.map(|x| x as f32), v));
        }
        for i in 0..face.len() {
            let j = (i + 1) % face.len();
            output.add_face(vec![outer[i], outer[j], inner[j], inner[i]], parents[fi], 0);
            output.add_face(
                vec![back_inner[i], back_inner[j], back_outer[j], back_outer[i]],
                parents[fi],
                1,
            );
            let (a, b) = (face[i], face[j]);
            if !open_edges.contains(&(a.min(b), a.max(b))) {
                output.add_face(
                    vec![outer[i], back_outer[i], back_outer[j], outer[j]],
                    parents[fi],
                    2,
                );
            }
            output.add_face(
                vec![inner[j], back_inner[j], back_inner[i], inner[i]],
                parents[fi],
                2,
            );
        }
    }
    Ok(output.finish())
}
