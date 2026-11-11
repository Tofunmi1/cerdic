//! Native Skin modifier.
//!
//! A skeleton tree is split into paths: from every joint a path continues along the bone that bends least, and the
//! other bones start new paths there. Each path becomes a closed tube of quad rings (a frame carried along the path
//! without twisting, radius pair as half extents). A single path is the result. Several paths are unioned with the
//! vendored Manifold library and the coplanar triangle pairs of the union are merged back into quads, so limbs and
//! branches stay mostly quads and only the junction curves are triangulated.

use std::collections::BTreeMap;

use crate::{
    BeveledMesh, ExecutionContext, KernelError, Mesh, MeshOp, MeshSelection, PolygonMesh,
    SkinJoint, SkinOptions, Solid,
};

type V3 = [f32; 3];

const MAX_VERTICES: usize = 200_000;

/// One tube: its quad faces, positions and the joint each vertex came from.
struct Tube {
    positions: Vec<V3>,
    origins: Vec<u32>,
    faces: Vec<Vec<u32>>,
}

/// Skins the graph natively, or returns `None` when this module does not implement the request.
#[must_use]
pub fn skin_graph(
    joints: &[SkinJoint],
    bones: &[[u32; 2]],
    options: &SkinOptions,
) -> Option<Result<BeveledMesh, KernelError>> {
    match run(joints, bones, options) {
        Ok(Some(mesh)) => Some(Ok(mesh)),
        Ok(None) => None,
        Err(error) => Some(Err(error)),
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "validation, path decomposition and assembly share bounded per-call state"
)]
fn run(
    joints: &[SkinJoint],
    bones: &[[u32; 2]],
    options: &SkinOptions,
) -> Result<Option<BeveledMesh>, KernelError> {
    // Large rigs (a full hand-and-finger body is about a hundred joints) have features of a few millimetres that the
    // later subdivision steps cannot take yet: the legacy build serves them.
    if joints.len() > 64 {
        return Ok(None);
    }
    if joints.len() > 4096 {
        return Err(KernelError::TooLarge);
    }
    if joints
        .iter()
        .any(|j| j.position.iter().any(|v| !v.is_finite()))
    {
        return Err(KernelError::NonFinite);
    }
    if joints
        .iter()
        .any(|j| j.radius.iter().any(|v| !v.is_finite() || *v < 0.0))
    {
        return Err(KernelError::InvalidArgument(
            "radii must be finite and non-negative",
        ));
    }
    // A radius of exactly zero in both directions is a point (a taper); flat rings (one zero) are left to the legacy build.
    if joints
        .iter()
        .any(|j| (j.radius[0] == 0.0) != (j.radius[1] == 0.0))
    {
        return Ok(None);
    }
    let mut adjacency = vec![Vec::<usize>::new(); joints.len()];
    for &[a, b] in bones {
        let (a, b) = (a as usize, b as usize);
        if a >= joints.len() || b >= joints.len() {
            return Err(KernelError::OutOfBounds);
        }
        if a == b {
            return Err(KernelError::InvalidArgument(
                "a bone cannot join a joint to itself",
            ));
        }
        if adjacency[a].contains(&b) {
            return Ok(None);
        }
        adjacency[a].push(b);
        adjacency[b].push(a);
    }
    // Components: each must be a tree with exactly one root joint.
    // A joint of no size can only be the tip of a limb: pinching a ring in the middle would tear the surface.
    if (0..joints.len()).any(|v| joints[v].radius == [0.0, 0.0] && adjacency[v].len() != 1) {
        return Ok(None);
    }
    let mut seen = vec![false; joints.len()];
    let mut paths: Vec<Vec<usize>> = Vec::new();
    let mut components: Vec<Vec<usize>> = Vec::new();
    for start in 0..joints.len() {
        if seen[start] {
            continue;
        }
        let mut component = Vec::new();
        let mut stack = vec![start];
        seen[start] = true;
        while let Some(v) = stack.pop() {
            component.push(v);
            for &n in &adjacency[v] {
                if !seen[n] {
                    seen[n] = true;
                    stack.push(n);
                }
            }
        }
        let edge_count = component.iter().map(|&v| adjacency[v].len()).sum::<usize>() / 2;
        if component.iter().filter(|&&v| joints[v].root).count() != 1 {
            return Err(KernelError::InvalidArgument(
                "every connected piece needs exactly one root joint",
            ));
        }
        if component.len() == 1 || edge_count + 1 != component.len() {
            return Ok(None);
        }
        paths.extend(split_into_paths(joints, &adjacency, &component));
        components.push(component);
    }
    // Branching skeletons get a hull junction (quads all the way) when the rings around each branch joint allow it.
    if adjacency.iter().any(|n| n.len() >= 3)
        && let Some(mesh) = hull_assembly(joints, &adjacency, &components, options)?
    {
        return Ok(Some(mesh));
    }
    let mut tubes = Vec::with_capacity(paths.len());
    let mut vertex_total = 0_usize;
    for path in &paths {
        let tube = build_tube(joints, path, options, [0.0, 0.0])?;
        vertex_total += tube.positions.len();
        if vertex_total > MAX_VERTICES {
            return Err(KernelError::TooLarge);
        }
        tubes.push(tube);
    }
    if tubes.len() == 1 {
        return Ok(Some(finish_single(tubes.remove(0))));
    }
    // The boolean union is only trusted on small skeletons: on a whole body the junction curves of many tubes leave
    // faces that the later subdivision steps cannot take, so the legacy build serves those.
    if joints.len() > 24 {
        return Ok(None);
    }
    union_tubes(joints, &tubes)
}

/// Splits one tree into paths that continue along the straightest bone at every joint.
fn split_into_paths(
    joints: &[SkinJoint],
    adjacency: &[Vec<usize>],
    component: &[usize],
) -> Vec<Vec<usize>> {
    let mut used: BTreeMap<(usize, usize), bool> = BTreeMap::new();
    let key = |a: usize, b: usize| (a.min(b), a.max(b));
    let direction = |from: usize, to: usize| unit(sub(joints[to].position, joints[from].position));
    let mut paths = Vec::new();
    // Start from end joints first, then from any joint that still has an unused bone.
    let mut starts: Vec<usize> = component
        .iter()
        .copied()
        .filter(|&v| adjacency[v].len() == 1)
        .collect();
    starts.extend(
        component
            .iter()
            .copied()
            .filter(|&v| adjacency[v].len() > 1),
    );
    for start in starts {
        loop {
            let first = adjacency[start]
                .iter()
                .copied()
                .find(|&n| !used.get(&key(start, n)).copied().unwrap_or(false));
            let Some(first) = first else { break };
            let mut path = vec![start, first];
            used.insert(key(start, first), true);
            loop {
                let (previous, here) = (path[path.len() - 2], path[path.len() - 1]);
                let heading = direction(previous, here);
                let next = adjacency[here]
                    .iter()
                    .copied()
                    .filter(|&n| !used.get(&key(here, n)).copied().unwrap_or(false))
                    .max_by(|&a, &b| {
                        dot(direction(here, a), heading)
                            .total_cmp(&dot(direction(here, b), heading))
                    });
                let Some(next) = next else { break };
                used.insert(key(here, next), true);
                path.push(next);
            }
            paths.push(path);
        }
    }
    paths
}

type Sample = (V3, [f32; 2], u32);

/// Cuts `from_start` off the front and `from_end` off the back of a sampled path, interpolating the new end samples.
/// `false` when nothing sensible would be left.
fn trim_samples(samples: &mut Vec<Sample>, from_start: f32, from_end: f32) -> bool {
    if from_start <= 0.0 && from_end <= 0.0 {
        return true;
    }
    let mut cumulative = vec![0.0_f32];
    for pair in samples.windows(2) {
        let last = cumulative.last().copied().unwrap_or(0.0);
        cumulative.push(last + norm(sub(pair[1].0, pair[0].0)));
    }
    let total = cumulative.last().copied().unwrap_or(0.0);
    if from_start + from_end >= total * 0.98 {
        return false;
    }
    let (low, high) = (from_start, total - from_end);
    let at = |distance: f32| -> Sample {
        let i = (0..samples.len() - 1)
            .find(|&i| cumulative[i + 1] >= distance)
            .unwrap_or(samples.len() - 2);
        let span = (cumulative[i + 1] - cumulative[i]).max(f32::EPSILON);
        let t = ((distance - cumulative[i]) / span).clamp(0.0, 1.0);
        let (a, b) = (samples[i], samples[i + 1]);
        (
            lerp(a.0, b.0, t),
            [a.1[0] * (1.0 - t) + b.1[0] * t, a.1[1] * (1.0 - t) + b.1[1] * t],
            if t <= 0.5 { a.2 } else { b.2 },
        )
    };
    let mut out = Vec::with_capacity(samples.len());
    out.push(if from_start > 0.0 { at(low) } else { samples[0] });
    for i in 1..samples.len() - 1 {
        if cumulative[i] > low + 1e-6 && cumulative[i] < high - 1e-6 {
            out.push(samples[i]);
        }
    }
    out.push(if from_end > 0.0 { at(high) } else { samples[samples.len() - 1] });
    *samples = out;
    samples.len() >= 2
}

/// A closed tube of quads along `path`. `trim` cuts that much off each end, which then stays open (no cap) so a junction
/// can be joined to it.
fn build_tube(
    joints: &[SkinJoint],
    path: &[usize],
    options: &SkinOptions,
    trim: [f32; 2],
) -> Result<Tube, KernelError> {
    let mut samples: Vec<Sample> = Vec::new();
    for pair in path.windows(2) {
        let (ja, jb) = (joints[pair[0]], joints[pair[1]]);
        let length = norm(sub(jb.position, ja.position));
        if length <= f32::EPSILON {
            return Err(KernelError::InvalidArgument("bone length must be positive"));
        }
        let widest = ja.radius[0]
            .max(ja.radius[1])
            .max(jb.radius[0])
            .max(jb.radius[1]);
        let inserts = if options.subdivide {
            ring_count(length, widest)
        } else {
            0
        };
        if samples.is_empty() {
            samples.push((ja.position, ja.radius, index(pair[0])));
        }
        for i in 1..=inserts {
            #[expect(clippy::cast_precision_loss, reason = "at most 64 inserted rings")]
            let t = i as f32 / (inserts + 1) as f32;
            samples.push((
                lerp(ja.position, jb.position, t),
                [
                    ja.radius[0] * (1.0 - t) + jb.radius[0] * t,
                    ja.radius[1] * (1.0 - t) + jb.radius[1] * t,
                ],
                index(if t <= 0.5 { pair[0] } else { pair[1] }),
            ));
        }
        samples.push((jb.position, jb.radius, index(pair[1])));
    }
    if !trim_samples(&mut samples, trim[0], trim[1]) {
        return Err(KernelError::InvalidArgument("a bone is too short for its junctions"));
    }
    let tangents: Vec<V3> = (0..samples.len())
        .map(|i| {
            if i == 0 {
                unit(sub(samples[1].0, samples[0].0))
            } else if i + 1 == samples.len() {
                unit(sub(samples[i].0, samples[i - 1].0))
            } else {
                unit(add(
                    unit(sub(samples[i].0, samples[i - 1].0)),
                    unit(sub(samples[i + 1].0, samples[i].0)),
                ))
            }
        })
        .collect();
    let t0 = tangents[0];
    let axis = if t0[0].abs() <= t0[1].abs() && t0[0].abs() <= t0[2].abs() {
        [1.0, 0.0, 0.0]
    } else if t0[1].abs() <= t0[2].abs() {
        [0.0, 1.0, 0.0]
    } else {
        [0.0, 0.0, 1.0]
    };
    let mut a = unit(cross(axis, t0));
    let (mut positions, mut origins) = (Vec::new(), Vec::new());
    let mut ring_start: Vec<u32> = Vec::with_capacity(samples.len());
    for i in 0..samples.len() {
        if i > 0 {
            a = rotate_minimal(a, tangents[i - 1], tangents[i]);
        }
        let b = cross(tangents[i], a);
        // Mitre: keep the thickness at a bend.
        let turn = if i == 0 || i + 1 == samples.len() {
            1.0
        } else {
            let (before, after) = (
                unit(sub(samples[i].0, samples[i - 1].0)),
                unit(sub(samples[i + 1].0, samples[i].0)),
            );
            (dot(before, after) * 0.5 + 0.5).sqrt().max(0.5)
        };
        let (p, radius, origin) = samples[i];
        ring_start.push(index(positions.len()));
        if radius == [0.0, 0.0] {
            // A ring of no size is one point (the tip of a taper).
            positions.push(p);
            origins.push(origin);
            continue;
        }
        for (sa, sb) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
            positions.push(add(
                add(p, mul(a, sa * radius[0] / turn)),
                mul(b, sb * radius[1] / turn),
            ));
            origins.push(origin);
        }
    }
    let pointed = |i: usize| samples[i].1 == [0.0, 0.0];
    let mut faces = Vec::new();
    for i in 0..samples.len() - 1 {
        let (base, next) = (ring_start[i], ring_start[i + 1]);
        for k in 0..4 {
            let k1 = (k + 1) % 4;
            match (pointed(i), pointed(i + 1)) {
                (false, false) => faces.push(vec![
                    base + index(k),
                    base + index(k1),
                    next + index(k1),
                    next + index(k),
                ]),
                (false, true) => faces.push(vec![base + index(k), base + index(k1), next]),
                (true, false) => faces.push(vec![base, next + index(k1), next + index(k)]),
                (true, true) => {}
            }
        }
    }
    if trim[0] <= 0.0 && !pointed(0) {
        faces.push(vec![3, 2, 1, 0]);
    }
    if trim[1] <= 0.0 && !pointed(samples.len() - 1) {
        let end = ring_start[samples.len() - 1];
        faces.push(vec![end, end + 1, end + 2, end + 3]);
    }
    Ok(Tube {
        positions,
        origins,
        faces,
    })
}

/// Extra rings along a bone so rings stay about two radii apart (at most 64).
fn ring_count(length: f32, radius: f32) -> usize {
    if radius <= 0.0 {
        return 0;
    }
    let wanted = (length / (2.0 * radius)).ceil();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to 0..=64 first"
    )]
    let count = wanted.clamp(1.0, 65.0) as usize;
    count - 1
}

fn finish_single(tube: Tube) -> BeveledMesh {
    let mut face_sizes = Vec::with_capacity(tube.faces.len());
    let mut corners = Vec::new();
    for face in &tube.faces {
        face_sizes.push(index(face.len()));
        corners.extend(face.iter().copied());
    }
    let count = face_sizes.len();
    BeveledMesh {
        positions: tube.positions,
        face_sizes,
        corners,
        vertex_origin: tube.origins,
        face_parent: vec![BeveledMesh::NO_ORIGIN; count],
        face_role: vec![0; count],
    }
}

/// Unions the tubes, then turns the triangles back into quads wherever two lie flat together.
fn union_tubes(joints: &[SkinJoint], tubes: &[Tube]) -> Result<Option<BeveledMesh>, KernelError> {
    let mut solids = Vec::with_capacity(tubes.len());
    for tube in tubes {
        let mut indices = Vec::new();
        for face in &tube.faces {
            // Quads and the quad caps fan into triangles.
            for k in 1..face.len() - 1 {
                indices.extend([face[0], face[k], face[k + 1]]);
            }
        }
        let Ok(solid) = Solid::from_mesh(&Mesh {
            positions: tube.positions.clone(),
            indices,
        }) else {
            return Ok(None);
        };
        solids.push(solid);
    }
    let context = ExecutionContext::new()?;
    // `union_all` takes 2 to 64 solids: fold larger sets in batches, the result going to the back of the queue.
    let mut queue: std::collections::VecDeque<Solid> = solids.into();
    while queue.len() > 1 {
        let take = queue.len().min(64);
        let batch: Vec<Solid> = queue.drain(..take).collect();
        let refs: Vec<&Solid> = batch.iter().collect();
        queue.push_back(Solid::union_all_with_context(&refs, &context)?);
    }
    let Some(solid) = queue.pop_front() else {
        return Ok(None);
    };
    let Ok(triangles) = solid.to_mesh() else {
        return Ok(None);
    };
    if triangles.indices.is_empty() || triangles.positions.len() > MAX_VERTICES {
        return Ok(None);
    }
    // The union leaves a few needle triangles where a ring vertex lands on another tube's surface; flip them away.
    let triangles = Mesh {
        indices: flip_slivers(&triangles.positions, triangles.indices),
        positions: triangles.positions,
    };
    // Where tube surfaces nearly touch the union leaves triangles a few micrometres across: weld what is closer than
    // that (faces that collapse go with it) so nothing later has to triangulate a hair.
    let extent = triangles
        .positions
        .iter()
        .flat_map(|p| p.iter().copied())
        .fold(0.0_f32, |m, c| m.max(c.abs()))
        .max(1e-6);
    let flat_sizes = vec![3_u32; triangles.indices.len() / 3];
    let welded = match crate::native::basic::edit(
        &PolygonMesh {
            positions: &triangles.positions,
            face_sizes: &flat_sizes,
            corners: &triangles.indices,
        },
        &MeshSelection::default(),
        &MeshOp::RemoveDoubles {
            distance: f64::from(extent) * 1e-5,
        },
    ) {
        Some(Ok(mesh)) => mesh,
        Some(Err(error)) => return Err(error),
        None => return Ok(None),
    };
    let triangles = Mesh {
        positions: welded.positions,
        indices: welded.corners,
    };
    if triangles.indices.is_empty() || triangles.indices.len() % 3 != 0 {
        return Ok(None);
    }
    let origins: Vec<u32> = triangles
        .positions
        .iter()
        .map(|&p| {
            let best = joints
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    norm(sub(a.position, p)).total_cmp(&norm(sub(b.position, p)))
                })
                .map_or(0, |(i, _)| i);
            index(best)
        })
        .collect();
    let sizes = vec![3_u32; triangles.indices.len() / 3];
    // Pass one joins the flat pairs (tube sides); pass two pairs what is left around the junctions into slightly
    // bent quads, which keeps the surface mostly quads.
    let join = |mesh: &PolygonMesh<'_>, face_angle: f64, shape_angle: f64| {
        crate::native::basic::edit(
            mesh,
            &MeshSelection::default(),
            &MeshOp::JoinTriangles {
                face_angle,
                shape_angle,
            },
        )
    };
    let Some(first) = join(
        &PolygonMesh {
            positions: &triangles.positions,
            face_sizes: &sizes,
            corners: &triangles.indices,
        },
        0.02,
        1.3,
    ) else {
        return Ok(None);
    };
    let first = first?;
    let mut merged = match join(
        &PolygonMesh {
            positions: &first.positions,
            face_sizes: &first.face_sizes,
            corners: &first.corners,
        },
        0.9,
        0.9,
    ) {
        Some(Ok(mut second)) => {
            // `vertex_origin` of the second pass indexes the first pass's vertices.
            for origin in &mut second.vertex_origin {
                *origin = first
                    .vertex_origin
                    .get(*origin as usize)
                    .copied()
                    .unwrap_or(BeveledMesh::NO_ORIGIN);
            }
            second
        }
        _ => first,
    };
    for origin in &mut merged.vertex_origin {
        *origin = origins
            .get(*origin as usize)
            .copied()
            .unwrap_or(BeveledMesh::NO_ORIGIN);
    }
    clean_faces(&mut merged);
    if !faces_are_sound(&merged) {
        // Something a later step could not triangulate: the legacy build serves it.
        return Ok(None);
    }
    let count = merged.face_sizes.len();
    // The union triangulates the curves where limbs meet (narrow forks have no room for a quad junction), so some
    // triangles remain there; the surface is still one closed manifold.
    merged.face_parent = vec![BeveledMesh::NO_ORIGIN; count];
    merged.face_role = vec![0; count];
    Ok(Some(merged))
}

fn index<T: TryInto<u32>>(value: T) -> u32 {
    value.try_into().unwrap_or(u32::MAX)
}
fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn mul(a: V3, s: f32) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(a: V3) -> f32 {
    dot(a, a).sqrt()
}
fn unit(a: V3) -> V3 {
    let n = norm(a);
    if n > f32::EPSILON {
        mul(a, 1.0 / n)
    } else {
        [1.0, 0.0, 0.0]
    }
}
fn lerp(a: V3, b: V3, t: f32) -> V3 {
    add(mul(a, 1.0 - t), mul(b, t))
}
/// Rotates `v` by the smallest rotation that takes direction `from` to direction `to`.
fn rotate_minimal(v: V3, from: V3, to: V3) -> V3 {
    let axis = cross(from, to);
    let s = norm(axis);
    let c = dot(from, to);
    if s < 1.0e-6 {
        return v;
    }
    let k = mul(axis, 1.0 / s);
    add(
        add(mul(v, c), mul(cross(k, v), s)),
        mul(k, dot(k, v) * (1.0 - c)),
    )
}

/// Paths that run between two special joints (an end of the skeleton or a branch), through joints with two bones.
fn segments_between_specials(adjacency: &[Vec<usize>], component: &[usize]) -> Vec<Vec<usize>> {
    let special = |v: usize| adjacency[v].len() != 2;
    let mut used: std::collections::BTreeSet<(usize, usize)> = std::collections::BTreeSet::new();
    let key = |a: usize, b: usize| (a.min(b), a.max(b));
    let mut paths = Vec::new();
    for &start in component {
        if !special(start) {
            continue;
        }
        for &first in &adjacency[start] {
            if used.contains(&key(start, first)) {
                continue;
            }
            used.insert(key(start, first));
            let mut path = vec![start, first];
            while !special(path[path.len() - 1]) {
                let (previous, here) = (path[path.len() - 2], path[path.len() - 1]);
                let Some(&next) = adjacency[here].iter().find(|&&n| n != previous) else {
                    break;
                };
                used.insert(key(here, next));
                path.push(next);
            }
            paths.push(path);
        }
    }
    paths
}

/// The skeleton as tubes that stop short of every branch joint, with the rings around each branch joint joined by a
/// convex hull (its ring faces dropped, its other faces merged into quads). `None` when a ring is not a face of its
/// hull (branches that leave at narrow angles), so the caller falls back to the boolean union.
#[expect(clippy::too_many_lines, reason = "tubes, junction hulls and quad merging share the vertex numbering")]
fn hull_assembly(
    joints: &[SkinJoint],
    adjacency: &[Vec<usize>],
    components: &[Vec<usize>],
    options: &SkinOptions,
) -> Result<Option<BeveledMesh>, KernelError> {
    let branch = |v: usize| adjacency[v].len() >= 3;
    // How far each branch joint's rings sit from it.
    // About 1.5 radii, but never more than 45% of the shortest bone leaving the joint (a bone between two branch joints
    // then still has a stretch of tube in the middle).
    let setback = |v: usize| {
        let shortest = adjacency[v]
            .iter()
            .map(|&n| norm(sub(joints[n].position, joints[v].position)))
            .fold(f32::INFINITY, f32::min);
        (1.5 * joints[v].radius[0].max(joints[v].radius[1])).min(0.45 * shortest)
    };
    let mut positions: Vec<V3> = Vec::new();
    let mut origins: Vec<u32> = Vec::new();
    let mut faces: Vec<Vec<u32>> = Vec::new();
    // For every branch joint, the ring (four global vertex ids) of each incident path end.
    let mut rings: BTreeMap<usize, Vec<[u32; 4]>> = BTreeMap::new();
    for component in components {
        for path in segments_between_specials(adjacency, component) {
            let (first, last) = (path[0], path[path.len() - 1]);
            let trim = [
                if branch(first) { setback(first) } else { 0.0 },
                if branch(last) { setback(last) } else { 0.0 },
            ];
            let Ok(tube) = build_tube(joints, &path, options, trim) else {
                return Ok(None);
            };
            let base = index(positions.len());
            let count = tube.positions.len();
            positions.extend(tube.positions.iter().copied());
            origins.extend(tube.origins.iter().copied());
            for face in &tube.faces {
                faces.push(face.iter().map(|&v| base + v).collect());
            }
            if branch(first) {
                rings.entry(first).or_default().push([base, base + 1, base + 2, base + 3]);
            }
            if branch(last) {
                let end = base + index(count - 4);
                rings.entry(last).or_default().push([end, end + 1, end + 2, end + 3]);
            }
        }
    }
    if positions.len() > MAX_VERTICES {
        return Err(KernelError::TooLarge);
    }
    for ring_set in rings.values() {
        let ids: Vec<u32> = ring_set.iter().flatten().copied().collect();
        let points: Vec<V3> = ids.iter().map(|&v| positions[v as usize]).collect();
        let Some(hull) = convex_hull(&points) else {
            return Ok(None);
        };
        // Every ring must be a face of the hull: two triangles drawn only from its own four vertices. Those are dropped
        // (a tube is joined there); the rest become the junction.
        let mut ring_triangles = vec![0_usize; ring_set.len()];
        let mut seen = vec![false; ids.len()];
        for triangle in &hull {
            for &corner in triangle {
                seen[corner] = true;
            }
            let ring_of = |corner: usize| corner / 4;
            if ring_of(triangle[0]) == ring_of(triangle[1]) && ring_of(triangle[1]) == ring_of(triangle[2]) {
                ring_triangles[ring_of(triangle[0])] += 1;
                continue;
            }
            faces.push(triangle.iter().map(|&corner| ids[corner]).collect());
        }
        if seen.iter().any(|&s| !s) || ring_triangles.iter().any(|&n| n < 2) {
            return Ok(None);
        }
    }
    // Quads wherever two triangles sit together: flat pairs first, then the bent pairs around the junctions.
    let face_sizes: Vec<u32> = faces.iter().map(|f| index(f.len())).collect();
    let corners: Vec<u32> = faces.iter().flatten().copied().collect();
    let flat_positions: Vec<V3> = positions.clone();
    let join = |mesh: &PolygonMesh<'_>, face_angle: f64, shape_angle: f64| {
        crate::native::basic::edit(
            mesh,
            &MeshSelection::default(),
            &MeshOp::JoinTriangles {
                face_angle,
                shape_angle,
            },
        )
    };
    let Some(first) = join(
        &PolygonMesh {
            positions: &flat_positions,
            face_sizes: &face_sizes,
            corners: &corners,
        },
        0.02,
        1.3,
    ) else {
        return Ok(None);
    };
    let first = first?;
    let mut merged = match join(
        &PolygonMesh {
            positions: &first.positions,
            face_sizes: &first.face_sizes,
            corners: &first.corners,
        },
        0.9,
        0.9,
    ) {
        Some(Ok(mut second)) => {
            for origin in &mut second.vertex_origin {
                *origin = first.vertex_origin.get(*origin as usize).copied().unwrap_or(BeveledMesh::NO_ORIGIN);
            }
            second
        }
        _ => first,
    };
    for origin in &mut merged.vertex_origin {
        *origin = origins.get(*origin as usize).copied().unwrap_or(BeveledMesh::NO_ORIGIN);
    }
    let count = merged.face_sizes.len();
    let quads = merged.face_sizes.iter().filter(|&&n| n == 4).count();
    if quads * 10 < count * 6 {
        return Ok(None);
    }
    clean_faces(&mut merged);
    if !faces_are_sound(&merged) {
        // Something a later step could not triangulate: the legacy build serves it.
        return Ok(None);
    }
    let count = merged.face_sizes.len();
    merged.face_parent = vec![BeveledMesh::NO_ORIGIN; count];
    merged.face_role = vec![0; count];
    Ok(Some(merged))
}

/// The convex hull of a point set as outward-wound triangles of point indices, or `None` for a flat or tiny set.
/// Coplanar points are nudged apart deterministically so that flat groups come out as triangle pairs.
fn convex_hull(points: &[V3]) -> Option<Vec<[usize; 3]>> {
    let n = points.len();
    if n < 4 {
        return None;
    }
    let extent = points
        .iter()
        .flat_map(|p| p.iter().copied())
        .fold(0.0_f32, |m, c| m.max(c.abs()))
        .max(1e-6);
    let p: Vec<[f64; 3]> = points
        .iter()
        .enumerate()
        .map(|(i, q)| {
            // A fixed pseudo-random nudge per point, a millionth of the extent.
            let h = |salt: u32| {
                let mut x = (i as u32).wrapping_mul(2_654_435_761).wrapping_add(salt.wrapping_mul(40_503));
                x ^= x >> 13;
                x = x.wrapping_mul(1_274_126_177);
                x ^= x >> 16;
                f64::from(x % 2001) / 1000.0 - 1.0
            };
            let scale = f64::from(extent) * 1e-6;
            [
                f64::from(q[0]) + h(1) * scale,
                f64::from(q[1]) + h(2) * scale,
                f64::from(q[2]) + h(3) * scale,
            ]
        })
        .collect();
    let d = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let cr = |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let len = |a: [f64; 3]| dot(a, a).sqrt();
    // A starting tetrahedron: far apart, then far from the line, then far from the plane.
    let i0 = (0..n).min_by(|&a, &b| p[a][0].total_cmp(&p[b][0]))?;
    let i1 = (0..n).max_by(|&a, &b| len(d(p[a], p[i0])).total_cmp(&len(d(p[b], p[i0]))))?;
    let line = d(p[i1], p[i0]);
    let i2 = (0..n).max_by(|&a, &b| len(cr(line, d(p[a], p[i0]))).total_cmp(&len(cr(line, d(p[b], p[i0])))))?;
    let normal = cr(line, d(p[i2], p[i0]));
    let i3 = (0..n).max_by(|&a, &b| dot(normal, d(p[a], p[i0])).abs().total_cmp(&dot(normal, d(p[b], p[i0])).abs()))?;
    if len(normal) < 1e-12 || dot(normal, d(p[i3], p[i0])).abs() < 1e-12 {
        return None;
    }
    let inside = [
        (p[i0][0] + p[i1][0] + p[i2][0] + p[i3][0]) / 4.0,
        (p[i0][1] + p[i1][1] + p[i2][1] + p[i3][1]) / 4.0,
        (p[i0][2] + p[i1][2] + p[i2][2] + p[i3][2]) / 4.0,
    ];
    let outward = |a: usize, b: usize, c: usize| -> [usize; 3] {
        let n = cr(d(p[b], p[a]), d(p[c], p[a]));
        if dot(n, d(inside, p[a])) > 0.0 { [a, c, b] } else { [a, b, c] }
    };
    let mut faces = vec![outward(i0, i1, i2), outward(i0, i1, i3), outward(i0, i2, i3), outward(i1, i2, i3)];
    for i in 0..n {
        if [i0, i1, i2, i3].contains(&i) {
            continue;
        }
        let visible: Vec<bool> = faces
            .iter()
            .map(|f| dot(cr(d(p[f[1]], p[f[0]]), d(p[f[2]], p[f[0]])), d(p[i], p[f[0]])) > 1e-14)
            .collect();
        if !visible.iter().any(|&v| v) {
            continue;
        }
        let mut edges: BTreeMap<(usize, usize), ()> = BTreeMap::new();
        for (f, &seen) in faces.iter().zip(&visible) {
            if seen {
                for k in 0..3 {
                    edges.insert((f[k], f[(k + 1) % 3]), ());
                }
            }
        }
        let horizon: Vec<(usize, usize)> = edges.keys().copied().filter(|&(u, v)| !edges.contains_key(&(v, u))).collect();
        let mut kept: Vec<[usize; 3]> = faces.iter().zip(&visible).filter(|entry| !*entry.1).map(|entry| *entry.0).collect();
        for (u, v) in horizon {
            kept.push([u, v, i]);
        }
        faces = kept;
    }
    Some(faces)
}

/// Flips the long edge of every needle-thin triangle with its neighbour, so each such triangle becomes two ordinary ones
/// (its third corner lies on that edge, so the neighbour splits there). A few passes settle chains of them.
fn flip_slivers(positions: &[V3], mut indices: Vec<u32>) -> Vec<u32> {
    let extent = positions
        .iter()
        .flat_map(|p| p.iter().copied())
        .fold(0.0_f32, |m, c| m.max(c.abs()))
        .max(1e-6);
    let corner = |i: u32| positions[i as usize].map(f64::from);
    let thin = |t: [u32; 3]| -> Option<usize> {
        // The corner (0..3) that lies almost on the opposite edge, if the triangle is a needle.
        let (a, b, c) = (corner(t[0]), corner(t[1]), corner(t[2]));
        let d = |p: [f64; 3], q: [f64; 3]| ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt();
        let edges = [d(b, c), d(c, a), d(a, b)];
        let (longest, &length) = edges.iter().enumerate().max_by(|x, y| x.1.total_cmp(y.1))?;
        let cross = [
            (b[1] - a[1]) * (c[2] - a[2]) - (b[2] - a[2]) * (c[1] - a[1]),
            (b[2] - a[2]) * (c[0] - a[0]) - (b[0] - a[0]) * (c[2] - a[2]),
            (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]),
        ];
        let height = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt() / length.max(1e-30);
        let _ = extent;
        (height < 2e-3 * length).then_some(longest)
    };
    for _ in 0..6 {
        let triangles = indices.len() / 3;
        let mut directed: BTreeMap<(u32, u32), usize> = BTreeMap::new();
        for t in 0..triangles {
            for k in 0..3 {
                directed.insert((indices[3 * t + k], indices[3 * t + (k + 1) % 3]), t);
            }
        }
        let mut changed = false;
        let mut touched = vec![false; triangles];
        for t in 0..triangles {
            let tri = [indices[3 * t], indices[3 * t + 1], indices[3 * t + 2]];
            let Some(opposite) = thin(tri) else { continue };
            if touched[t] {
                continue;
            }
            // `opposite` is the corner across from the longest edge: the edge is the other two corners, a -> b.
            let (c, a, b) = (tri[opposite], tri[(opposite + 1) % 3], tri[(opposite + 2) % 3]);
            let Some(&n) = directed.get(&(b, a)) else { continue };
            if touched[n] {
                continue;
            }
            let neighbour = [indices[3 * n], indices[3 * n + 1], indices[3 * n + 2]];
            let Some(k) = neighbour.iter().position(|&v| v == b) else { continue };
            let d = neighbour[(k + 2) % 3];
            // (a, b, c) and (b, a, d) become (a, d, c) and (d, b, c).
            indices[3 * t..3 * t + 3].copy_from_slice(&[a, d, c]);
            indices[3 * n..3 * n + 3].copy_from_slice(&[d, b, c]);
            touched[t] = true;
            touched[n] = true;
            changed = true;
        }
        if !changed {
            break;
        }
    }
    indices
}

/// Final tidy of a finished skin mesh so later steps can triangulate every face: twisted quads become two triangles, and
/// needle triangles (a corner that is almost a straight line) are flipped against their neighbour, splitting a quad
/// neighbour first when needed.
fn clean_faces(mesh: &mut BeveledMesh) {
    let mut faces: Vec<Vec<u32>> = Vec::with_capacity(mesh.face_sizes.len());
    let mut at = 0;
    for &size in &mesh.face_sizes {
        faces.push(mesh.corners[at..at + size as usize].to_vec());
        at += size as usize;
    }
    let point = |i: u32| mesh.positions[i as usize].map(f64::from);
    let dist = |a: u32, b: u32| {
        let (p, q) = (point(a), point(b));
        ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
    };
    // How far the fourth corner of a quad lies off the plane of the other three, relative to the longest side.
    let twisted = |q: &[u32]| {
        let (a, b, c, d) = (point(q[0]), point(q[1]), point(q[2]), point(q[3]));
        let (u, v, w) = (
            [b[0] - a[0], b[1] - a[1], b[2] - a[2]],
            [c[0] - a[0], c[1] - a[1], c[2] - a[2]],
            [d[0] - a[0], d[1] - a[1], d[2] - a[2]],
        );
        let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-30);
        let off = (n[0] * w[0] + n[1] * w[1] + n[2] * w[2]).abs() / length;
        // The polygon triangulation later wants a quad to be flat within 0.1% of its extent and each corner's triangle
        // to have area above a fixed share of extent squared; split anything that is close to failing either.
        let points = [a, b, c, d];
        let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for p in &points {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let extent = ((hi[0] - lo[0]).powi(2) + (hi[1] - lo[1]).powi(2) + (hi[2] - lo[2]).powi(2)).sqrt();
        let thin = (0..4).any(|i| {
            let (p, m, r) = (points[(i + 3) % 4], points[i], points[(i + 1) % 4]);
            let (e1, e2) = ([m[0] - p[0], m[1] - p[1], m[2] - p[2]], [r[0] - m[0], r[1] - m[1], r[2] - m[2]]);
            let cr = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
            (cr[0] * cr[0] + cr[1] * cr[1] + cr[2] * cr[2]).sqrt() / 2.0 < 3e-3 * extent * extent
        });
        off > 4e-4 * extent || thin
    };
    // The corner of a triangle that sits nearly on the line of the opposite (longest) edge, if it is a needle.
    let needle = |t: &[u32]| -> Option<usize> {
        let edges = [dist(t[1], t[2]), dist(t[2], t[0]), dist(t[0], t[1])];
        let (longest, &length) = edges.iter().enumerate().max_by(|x, y| x.1.total_cmp(y.1))?;
        let (a, b, c) = (point(t[0]), point(t[1]), point(t[2]));
        let cross = [
            (b[1] - a[1]) * (c[2] - a[2]) - (b[2] - a[2]) * (c[1] - a[1]),
            (b[2] - a[2]) * (c[0] - a[0]) - (b[0] - a[0]) * (c[2] - a[2]),
            (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]),
        ];
        let height = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt() / length.max(1e-30);
        (height < 2e-3 * length).then_some(longest)
    };
    for _ in 0..8 {
        let mut changed = false;
        // Twisted quads become triangles along their shorter diagonal.
        let mut next: Vec<Vec<u32>> = Vec::with_capacity(faces.len());
        for face in faces.drain(..) {
            if face.len() == 4 && twisted(&face) {
                let (d02, d13) = (dist(face[0], face[2]), dist(face[1], face[3]));
                if d02 <= d13 {
                    next.push(vec![face[0], face[1], face[2]]);
                    next.push(vec![face[0], face[2], face[3]]);
                } else {
                    next.push(vec![face[0], face[1], face[3]]);
                    next.push(vec![face[1], face[2], face[3]]);
                }
                changed = true;
            } else {
                next.push(face);
            }
        }
        faces = next;
        // Which face walks each directed edge.
        let mut walker: BTreeMap<(u32, u32), usize> = BTreeMap::new();
        for (f, face) in faces.iter().enumerate() {
            for i in 0..face.len() {
                walker.insert((face[i], face[(i + 1) % face.len()]), f);
            }
        }
        let mut touched = vec![false; faces.len()];
        let mut split: Vec<usize> = Vec::new();
        let mut flips: Vec<(usize, usize, [u32; 3], [u32; 3])> = Vec::new();
        for t in 0..faces.len() {
            if faces[t].len() != 3 || touched[t] {
                continue;
            }
            let Some(opposite) = needle(&faces[t]) else { continue };
            let tri = [faces[t][0], faces[t][1], faces[t][2]];
            let (c, a, b) = (tri[opposite], tri[(opposite + 1) % 3], tri[(opposite + 2) % 3]);
            let Some(&n) = walker.get(&(b, a)) else { continue };
            if touched[n] {
                continue;
            }
            if faces[n].len() == 4 {
                touched[n] = true;
                split.push(n);
            } else if faces[n].len() == 3 {
                let neighbour = &faces[n];
                let Some(k) = neighbour.iter().position(|&v| v == b) else { continue };
                let d = neighbour[(k + 2) % 3];
                touched[t] = true;
                touched[n] = true;
                flips.push((t, n, [a, d, c], [d, b, c]));
            }
        }
        for (t, n, first, second) in flips {
            faces[t] = first.to_vec();
            faces[n] = second.to_vec();
            changed = true;
        }
        for n in split {
            let q = faces[n].clone();
            let (d02, d13) = (dist(q[0], q[2]), dist(q[1], q[3]));
            if d02 <= d13 {
                faces[n] = vec![q[0], q[1], q[2]];
                faces.push(vec![q[0], q[2], q[3]]);
            } else {
                faces[n] = vec![q[0], q[1], q[3]];
                faces.push(vec![q[1], q[2], q[3]]);
            }
            changed = true;
        }
        if !changed {
            break;
        }
    }
    mesh.face_sizes = faces.iter().map(|f| index(f.len())).collect();
    mesh.corners = faces.iter().flatten().copied().collect();
}

/// Whether every face is something a later step can triangulate: no repeated corner, no needle or zero-area face, no
/// corner that bends against the face's own normal.
fn faces_are_sound(mesh: &BeveledMesh) -> bool {
    let mut at = 0;
    for &size in &mesh.face_sizes {
        let c = &mesh.corners[at..at + size as usize];
        at += size as usize;
        if (0..c.len()).any(|i| c[..i].contains(&c[i])) {
            return false;
        }
        let p = |i: u32| mesh.positions[i as usize].map(f64::from);
        let mut normal = [0.0_f64; 3];
        let mut longest = 0.0_f64;
        for i in 0..c.len() {
            let (a, b) = (p(c[i]), p(c[(i + 1) % c.len()]));
            normal[0] += (a[1] - b[1]) * (a[2] + b[2]);
            normal[1] += (a[2] - b[2]) * (a[0] + b[0]);
            normal[2] += (a[0] - b[0]) * (a[1] + b[1]);
            longest = longest.max(((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt());
        }
        let scale = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
        // Too small relative to its own size, or in absolute terms (the polygon triangulation later uses a fixed floor).
        if scale / 2.0 < 1e-6 * longest * longest || scale / 2.0 < 1e-8 {
            return false;
        }
        for i in 0..c.len() {
            let (a, b, d) = (p(c[i]), p(c[(i + 1) % c.len()]), p(c[(i + 2) % c.len()]));
            let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let e2 = [d[0] - b[0], d[1] - b[1], d[2] - b[2]];
            let turn = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
            let (l1, l2) = (
                (e1[0] * e1[0] + e1[1] * e1[1] + e1[2] * e1[2]).sqrt(),
                (e2[0] * e2[0] + e2[1] * e2[1] + e2[2] * e2[2]).sqrt(),
            );
            let along = (turn[0] * normal[0] + turn[1] * normal[1] + turn[2] * normal[2]) / scale;
            // Straight (a needle) or bending the wrong way.
            if along < 1e-3 * l1 * l2 {
                return false;
            }
        }
    }
    true
}
