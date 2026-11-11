//! Native spin and loop cut mesh operators.

use std::collections::{BTreeMap, BTreeSet};

use super::edit::{NO_ORIGIN, Soup};
use crate::{EditedMesh, KernelError, MeshOp, MeshSelection, PolygonMesh};

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
fn point(s: &Soup, v: u32) -> V3 {
    s.positions[v as usize].map(f64::from)
}
#[expect(
    clippy::cast_possible_truncation,
    reason = "mesh positions are stored as f32"
)]
fn f32x3(p: V3) -> [f32; 3] {
    [p[0] as f32, p[1] as f32, p[2] as f32]
}
fn index(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(NO_ORIGIN)
}

fn spin(
    mut s: Soup,
    selection: &MeshSelection<'_>,
    center: V3,
    axis: V3,
    angle: f64,
    steps: u32,
    merge: bool,
    distance: f64,
) -> Result<EditedMesh, KernelError> {
    if center.iter().chain(axis.iter()).any(|v| !v.is_finite())
        || !angle.is_finite()
        || !distance.is_finite()
    {
        return Err(KernelError::NonFinite);
    }
    if norm(axis) < 1e-20 || steps == 0 || distance < 0.0 || angle == 0.0 {
        return Err(KernelError::InvalidArgument("rejected by the kernel"));
    }
    if selection.faces.iter().any(|&f| f as usize >= s.faces.len()) {
        return Err(KernelError::OutOfBounds);
    }
    let edge_faces = s.edge_faces();
    let selected: BTreeSet<usize> = if selection.faces.is_empty() {
        (0..s.faces.len()).collect()
    } else {
        selection.faces.iter().map(|&f| f as usize).collect()
    };
    let mut oriented = Vec::new();
    if selection.edges.is_empty() {
        for (fi, face) in s.faces.iter().enumerate() {
            if !selected.contains(&fi) {
                continue;
            }
            for i in 0..face.len() {
                let (a, b) = (face[i], face[(i + 1) % face.len()]);
                let users = &edge_faces[&(a.min(b), a.max(b))];
                if users
                    .iter()
                    .filter(|(other, _)| selected.contains(other))
                    .count()
                    == 1
                {
                    oriented.push((a, b));
                }
            }
        }
    } else {
        for &[a, b] in selection.edges {
            if a as usize >= s.positions.len() || b as usize >= s.positions.len() {
                return Err(KernelError::OutOfBounds);
            }
            if !edge_faces.contains_key(&(a.min(b), a.max(b))) {
                return Err(KernelError::InvalidArgument("rejected by the kernel"));
            }
            oriented.push((a, b));
        }
    }
    if oriented.is_empty() {
        return Err(KernelError::InvalidArgument("rejected by the kernel"));
    }
    let original = s.positions.clone();
    let axis = unit(axis);
    let full = (angle.abs() - std::f64::consts::TAU).abs() <= 1e-6;
    let mut rings = vec![vec![0_u32; original.len()]; steps as usize + 1];
    for (i, slot) in rings[0].iter_mut().enumerate() {
        *slot = index(i);
    }
    for step in 1..=steps {
        let theta = angle * f64::from(step) / f64::from(steps);
        let (sn, cs) = theta.sin_cos();
        for (vi, p) in original.iter().enumerate() {
            let rel = sub(p.map(f64::from), center);
            let rotated = add(
                center,
                add(
                    add(mul(rel, cs), mul(cross(axis, rel), sn)),
                    mul(axis, dot(axis, rel) * (1.0 - cs)),
                ),
            );
            let weld =
                merge && full && step == steps && norm(sub(rotated, p.map(f64::from))) <= distance;
            rings[step as usize][vi] = if weld {
                index(vi)
            } else {
                s.add_vertex(f32x3(rotated), s.vertex_origin[vi])
            };
        }
    }
    let old_faces = s.faces.clone();
    let parents = s.face_parent.clone();
    let roles = s.face_role.clone();
    let keep_profile = !selection.edges.is_empty() || (full && !merge);
    let mut faces = Vec::new();
    let mut fp = Vec::new();
    let mut fr = Vec::new();
    for (fi, face) in old_faces.into_iter().enumerate() {
        if keep_profile || !selected.contains(&fi) {
            faces.push(face);
            fp.push(parents[fi]);
            fr.push(roles[fi]);
        }
    }
    for step in 0..steps as usize {
        for &(a, b) in &oriented {
            let a0 = rings[step][a as usize];
            let b0 = rings[step][b as usize];
            let a1 = rings[step + 1][a as usize];
            let b1 = rings[step + 1][b as usize];
            let parent = edge_faces
                .get(&(a.min(b), a.max(b)))
                .and_then(|u| u.first())
                .map_or(NO_ORIGIN, |(f, _)| index(*f));
            let on_axis = |v: u32| {
                let d = sub(point(&s, v), center);
                norm(sub(d, mul(axis, dot(d, axis)))) <= distance.max(1e-8)
            };
            let (face, role) = if on_axis(a) && on_axis(b) {
                continue;
            } else if on_axis(a) {
                (vec![a0, b0, b1], 1)
            } else if on_axis(b) {
                (vec![a0, b0, a1], 1)
            } else if a0 == a1 && b0 == b1 {
                continue;
            } else {
                (vec![a0, b0, b1, a1], 1)
            };
            let mut face = face;
            face.reverse();
            faces.push(face);
            fp.push(parent);
            fr.push(role);
        }
    }
    s.faces = faces;
    s.face_parent = fp;
    s.face_role = fr;
    Ok(s.finish())
}

fn loop_cut(mut s: Soup, at: V3, cuts: u32, smooth: f64) -> Result<EditedMesh, KernelError> {
    if at.iter().any(|v| !v.is_finite()) || !smooth.is_finite() {
        return Err(KernelError::NonFinite);
    }
    if !(1..=32).contains(&cuts) || !(0.0..=1.0).contains(&smooth) {
        return Err(KernelError::InvalidArgument("rejected by the kernel"));
    }
    let edge_faces = s.edge_faces();
    let distance =
        |edge: &(u32, u32)| segment_distance_sq(at, point(&s, edge.0), point(&s, edge.1));
    let start = edge_faces
        .keys()
        .copied()
        .min_by(|a, b| distance(a).total_cmp(&distance(b)).then_with(|| a.cmp(b)))
        .ok_or(KernelError::OperatorRefused)?;
    let mut opposite = BTreeMap::new();
    let mut face_pair = BTreeMap::new();
    for (fi, face) in s.faces.iter().enumerate() {
        if face.len() != 4 {
            continue;
        }
        for i in 0..2 {
            let a = (
                face[i].min(face[(i + 1) % 4]),
                face[i].max(face[(i + 1) % 4]),
            );
            let b = (
                face[(i + 2) % 4].min(face[(i + 3) % 4]),
                face[(i + 2) % 4].max(face[(i + 3) % 4]),
            );
            opposite.entry(a).or_insert_with(BTreeSet::new).insert(b);
            opposite.entry(b).or_insert_with(BTreeSet::new).insert(a);
            if i == 0 {
                let e1 = (face[1].min(face[2]), face[1].max(face[2]));
                let e3 = (face[3].min(face[0]), face[3].max(face[0]));
                face_pair.insert(fi, [(a, b), (e1, e3)]);
            }
        }
    }
    let mut ring = BTreeSet::new();
    let mut todo = vec![start];
    while let Some(edge) = todo.pop() {
        if !ring.insert(edge) {
            continue;
        }
        let users = edge_faces.get(&edge).ok_or(KernelError::OperatorRefused)?;
        if users.len() != 2 {
            return Err(KernelError::OperatorRefused);
        }
        for &(fi, _) in users {
            if s.faces[fi].len() != 4 {
                return Err(KernelError::OperatorRefused);
            }
            let pairs = face_pair[&fi];
            let other = pairs
                .iter()
                .find_map(|&(a, b)| {
                    if a == edge {
                        Some(b)
                    } else if b == edge {
                        Some(a)
                    } else {
                        None
                    }
                })
                .ok_or(KernelError::OperatorRefused)?;
            todo.push(other);
        }
    }
    if ring
        .iter()
        .any(|edge| opposite.get(edge).map_or(true, BTreeSet::is_empty))
    {
        return Err(KernelError::OperatorRefused);
    }
    let mut splits = BTreeMap::new();
    let edge_users = s.edge_faces();
    for &(a, b) in &ring {
        let pa = point(&s, a);
        let pb = point(&s, b);
        let mut ids = Vec::new();
        for i in 1..=cuts {
            let t = f64::from(i) / f64::from(cuts + 1);
            let push = edge_users
                .get(&(a.min(b), a.max(b)))
                .map_or([0.0; 3], |users| s.arc_push(users, a, b, t, smooth));
            ids.push(s.add_vertex(f32x3(add(add(mul(pa, 1.0 - t), mul(pb, t)), push)), NO_ORIGIN));
        }
        splits.insert((a, b), ids);
    }
    let old = s.faces.clone();
    let old_parent = s.face_parent.clone();
    let old_role = s.face_role.clone();
    let mut faces = Vec::new();
    let mut parents = Vec::new();
    let mut roles = Vec::new();
    for (fi, face) in old.iter().enumerate() {
        let pair = face_pair.get(&fi).and_then(|pairs| {
            pairs
                .iter()
                .find(|(a, b)| ring.contains(a) && ring.contains(b))
        });
        let Some(&(e0, e2)) = pair else {
            faces.push(face.clone());
            parents.push(old_parent[fi]);
            roles.push(old_role[fi]);
            continue;
        };
        let i0 = (0..4)
            .find(|&i| {
                (
                    face[i].min(face[(i + 1) % 4]),
                    face[i].max(face[(i + 1) % 4]),
                ) == e0
            })
            .ok_or(KernelError::OperatorRefused)?;
        let i2 = (0..4)
            .find(|&i| {
                (
                    face[i].min(face[(i + 1) % 4]),
                    face[i].max(face[(i + 1) % 4]),
                ) == e2
            })
            .ok_or(KernelError::OperatorRefused)?;
        if (i0 + 2) % 4 != i2 {
            return Err(KernelError::OperatorRefused);
        }
        let (a, b, c, d) = (
            face[i0],
            face[(i0 + 1) % 4],
            face[(i0 + 2) % 4],
            face[(i0 + 3) % 4],
        );
        let p0 = &splits[&e0];
        let p2 = &splits[&e2];
        let mut p = vec![a];
        if a < b {
            p.extend(p0.iter().copied());
        } else {
            p.extend(p0.iter().rev().copied());
        }
        p.push(b);
        let mut q = vec![d];
        if c < d {
            q.extend(p2.iter().rev().copied());
        } else {
            q.extend(p2.iter().copied());
        }
        q.push(c);
        for k in 0..=cuts as usize {
            faces.push(vec![p[k], p[k + 1], q[k + 1], q[k]]);
            parents.push(old_parent[fi]);
            roles.push(1);
        }
    }
    let _ = smooth;
    s.faces = faces;
    s.face_parent = parents;
    s.face_role = roles;
    Ok(s.finish())
}

fn segment_distance_sq(p: V3, a: V3, b: V3) -> f64 {
    let ab = sub(b, a);
    let len = dot(ab, ab);
    let t = if len > 1e-24 {
        (dot(sub(p, a), ab) / len).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let delta = sub(p, add(a, mul(ab, t)));
    dot(delta, delta)
}

/// Runs the native implementation, or returns `None` when another module owns the operator.
#[must_use]
pub fn edit(
    mesh: &PolygonMesh<'_>,
    selection: &MeshSelection<'_>,
    op: &MeshOp,
) -> Option<Result<EditedMesh, KernelError>> {
    match *op {
        MeshOp::Spin {
            center,
            axis,
            angle,
            steps,
            merge,
            distance,
        } => Some(
            Soup::from_mesh(mesh)
                .and_then(|s| spin(s, selection, center, axis, angle, steps, merge, distance)),
        ),
        MeshOp::LoopCut { at, cuts, smooth } => {
            Some(Soup::from_mesh(mesh).and_then(|s| loop_cut(s, at, cuts, smooth)))
        }
        _ => None,
    }
}
