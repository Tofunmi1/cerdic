//! Native cleanup and edge-loop operators.
use std::collections::{BTreeMap, BTreeSet};

use super::edit::Soup;
use crate::{EditedMesh, KernelError, MeshOp, MeshSelection, PolygonMesh};

type V = [f64; 3];
fn add(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn mul(a: V, k: f64) -> V {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn dot(a: V, b: V) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V, b: V) -> V {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn len(a: V) -> f64 {
    dot(a, a).sqrt()
}
fn unit(a: V) -> V {
    let n = len(a);
    if n > 1e-20 { mul(a, 1.0 / n) } else { [0.0; 3] }
}
fn p(s: &Soup, i: u32) -> V {
    s.positions[i as usize].map(f64::from)
}
#[expect(
    clippy::cast_possible_truncation,
    reason = "kernel coordinates are f32"
)]
fn f(v: V) -> [f32; 3] {
    [v[0] as f32, v[1] as f32, v[2] as f32]
}
fn angle_score(s: &Soup, t: &[Vec<u32>; 2]) -> f64 {
    let mut smallest = f64::INFINITY;
    for tri in t {
        for i in 0..3 {
            let a = unit(sub(p(s, tri[(i + 1) % 3]), p(s, tri[i])));
            let b = unit(sub(p(s, tri[(i + 2) % 3]), p(s, tri[i])));
            smallest = smallest.min(dot(a, b).clamp(-1.0, 1.0).acos());
        }
    }
    smallest
}
fn convex(s: &Soup, q: [u32; 4]) -> bool {
    let n = unit(cross(
        sub(p(s, q[1]), p(s, q[0])),
        sub(p(s, q[2]), p(s, q[0])),
    ));
    let mut sign = 0.0_f64;
    for i in 0..4 {
        let a = p(s, q[i]);
        let b = p(s, q[(i + 1) % 4]);
        let c = p(s, q[(i + 2) % 4]);
        let d = dot(cross(sub(b, a), sub(c, b)), n);
        if d.abs() < 1e-12 {
            return false;
        }
        if sign == 0.0 {
            sign = d.signum()
        } else if d * sign <= 0.0 {
            return false;
        }
    }
    true
}
fn beautify(mut s: Soup, by_angle: bool) -> Result<EditedMesh, KernelError> {
    for _ in 0..32 {
        let edges = s.edge_faces();
        let mut changed = false;
        for (&(a, b), users) in &edges {
            if users.len() != 2 {
                continue;
            }
            let (fa, ia) = users[0];
            let (fb, _ib) = users[1];
            if s.faces[fa].len() != 3 || s.faces[fb].len() != 3 {
                continue;
            }
            let (u, v) = (s.faces[fa][ia], s.faces[fa][(ia + 1) % 3]);
            let x = s.faces[fa].iter().copied().find(|w| *w != u && *w != v);
            let y = s.faces[fb].iter().copied().find(|w| *w != u && *w != v);
            let (Some(x), Some(y)) = (x, y) else { continue };
            if x == y || edges.contains_key(&(x.min(y), x.max(y))) {
                continue;
            }
            let q = [x, u, y, v];
            if !convex(&s, q) {
                continue;
            }
            let old = [s.faces[fa].clone(), s.faces[fb].clone()];
            let new = [vec![x, u, y], vec![x, y, v]];
            let improve = if by_angle {
                angle_score(&s, &new) > angle_score(&s, &[old[0].clone(), old[1].clone()]) + 1e-10
            } else {
                distance(p(&s, x), p(&s, y)) + 1e-10 < distance(p(&s, u), p(&s, v))
            };
            if improve {
                s.faces[fa] = new[0].clone();
                s.faces[fb] = new[1].clone();
                changed = true;
                break;
            }
            let _ = (a, b);
        }
        if !changed {
            break;
        }
    }
    Ok(s.finish())
}
fn planar(mut s: Soup, iters: u32, factor: f64) -> Result<EditedMesh, KernelError> {
    if !factor.is_finite() {
        return Err(KernelError::NonFinite);
    }
    if !(0.0..=1.0).contains(&factor) || iters > 50 {
        return Err(KernelError::InvalidArgument("rejected by the kernel"));
    }
    for _ in 0..iters {
        let mut shifts: BTreeMap<u32, (V, u32)> = BTreeMap::new();
        for face in &s.faces {
            if face.len() < 4 {
                continue;
            }
            let mut c = [0.0; 3];
            for &v in face {
                c = add(c, p(&s, v));
            }
            #[expect(
                clippy::cast_precision_loss,
                reason = "a polygon has a practical number of corners"
            )]
            let count = face.len() as f64;
            c = mul(c, 1.0 / count);
            let mut n = [0.0; 3];
            for i in 0..face.len() {
                let a = p(&s, face[i]);
                let b = p(&s, face[(i + 1) % face.len()]);
                n = add(n, cross(a, b));
            }
            n = unit(n);
            if len(n) < 0.5 {
                continue;
            }
            let mut request = Vec::new();
            for &v in face {
                let here = p(&s, v);
                request.push((v, mul(n, -dot(sub(here, c), n) * factor)));
            }
            let projected: Vec<V> = face
                .iter()
                .map(|&v| add(p(&s, v), mul(n, -dot(sub(p(&s, v), c), n) * factor)))
                .collect();
            let mut nn = [0.0; 3];
            for i in 0..projected.len() {
                nn = add(
                    nn,
                    cross(
                        sub(projected[i], c),
                        sub(projected[(i + 1) % projected.len()], c),
                    ),
                );
            }
            if len(nn) < 1e-12 {
                continue;
            }
            for (v, d) in request {
                let e = shifts.entry(v).or_insert(([0.0; 3], 0));
                e.0 = add(e.0, d);
                e.1 += 1;
            }
        }
        for (v, (d, n)) in shifts {
            if n > 0 {
                s.positions[v as usize] = f(add(p(&s, v), mul(d, 1.0 / f64::from(n))));
            }
        }
    }
    Ok(s.finish())
}
fn loops(s: &Soup, selected: &[[u32; 2]]) -> Result<Vec<Vec<u32>>, KernelError> {
    if selected.is_empty() {
        return Err(KernelError::InvalidArgument("rejected by the kernel"));
    }
    let edges = s.edge_faces();
    let mut adj: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    let mut unique = BTreeSet::new();
    for &[a, b] in selected {
        if a as usize >= s.positions.len() || b as usize >= s.positions.len() {
            return Err(KernelError::OutOfBounds);
        }
        let k = (a.min(b), a.max(b));
        if !edges.contains_key(&k) {
            return Err(KernelError::InvalidArgument("rejected by the kernel"));
        }
        if unique.insert(k) {
            adj.entry(a).or_default().push(b);
            adj.entry(b).or_default().push(a);
        }
    }
    if adj.values().any(|n| n.len() > 2) {
        return Err(KernelError::OperatorRefused);
    }
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for &start in adj.keys() {
        if seen.contains(&start) {
            continue;
        }
        let mut component = Vec::new();
        let mut stack = vec![start];
        seen.insert(start);
        while let Some(v) = stack.pop() {
            component.push(v);
            for &w in &adj[&v] {
                if seen.insert(w) {
                    stack.push(w)
                }
            }
        }
        let first = component
            .iter()
            .copied()
            .filter(|v| adj[v].len() == 1)
            .min()
            .unwrap_or_else(|| *component.iter().min().unwrap_or(&start));
        let mut chain = vec![first];
        let mut prev = None;
        let mut cur = first;
        loop {
            let next = adj[&cur].iter().copied().find(|v| {
                Some(*v) != prev && (!chain.contains(v) || (*v == first && component.len() > 2))
            });
            let Some(n) = next else { break };
            if n == first {
                break;
            }
            chain.push(n);
            prev = Some(cur);
            cur = n;
        }
        out.push(chain);
    }
    Ok(out)
}
fn distance(a: V, b: V) -> f64 {
    len(sub(a, b))
}
fn resample(s: &Soup, chain: &[u32], closed: bool) -> Vec<V> {
    let pts: Vec<V> = chain.iter().map(|&v| p(s, v)).collect();
    let n = pts.len();
    let segs = if closed { n } else { n.saturating_sub(1) };
    let mut cumulative = vec![0.0];
    for i in 0..segs {
        cumulative.push(cumulative[i] + distance(pts[i], pts[(i + 1) % n]));
    }
    let total = *cumulative.last().unwrap_or(&0.0);
    let count = if closed { n } else { n.saturating_sub(1) };
    let mut out = Vec::new();
    for j in 0..count {
        let target = if closed {
            total * j as f64 / n as f64
        } else {
            total * j as f64 / (n.saturating_sub(1).max(1)) as f64
        };
        let seg = (0..segs)
            .find(|&i| cumulative[i + 1] >= target)
            .unwrap_or(segs.saturating_sub(1));
        let span = (cumulative[seg + 1] - cumulative[seg]).max(1e-20);
        let t = (target - cumulative[seg]) / span;
        out.push(add(mul(pts[seg], 1.0 - t), mul(pts[(seg + 1) % n], t)));
    }
    if !closed && n > 1 {
        out.push(pts[n - 1]);
    }
    out
}
/// If the closed loop lies on a circle (a least-squares fit in its plane, every point within 0.1% of the radius), the
/// loop's vertices moved to equal angles around that circle, starting at the first vertex and keeping the loop's direction.
#[expect(
    clippy::cast_precision_loss,
    clippy::many_single_char_names,
    reason = "loop indices are bounded by mesh size; the fit uses the usual x, y, D, E, F names"
)]
fn fit_circle(points: &[V]) -> Option<Vec<V>> {
    let n = points.len();
    let centroid = mul(points.iter().fold([0.0; 3], |a, &q| add(a, q)), 1.0 / n as f64);
    let normal = unit(points.iter().enumerate().fold([0.0; 3], |acc, (i, &q)| {
        add(acc, cross(sub(q, centroid), sub(points[(i + 1) % n], centroid)))
    }));
    if len(normal) < 0.5 {
        return None;
    }
    let first = sub(points[0], centroid);
    let e1 = unit(sub(first, mul(normal, dot(first, normal))));
    let e2 = cross(normal, e1);
    if len(e1) < 0.5 {
        return None;
    }
    let xy: Vec<(f64, f64)> = points
        .iter()
        .map(|&q| {
            let d = sub(q, centroid);
            (dot(d, e1), dot(d, e2))
        })
        .collect();
    // Minimise sum (x^2 + y^2 + D x + E y + F)^2: a 3 by 3 linear system.
    let (mut sxx, mut sxy, mut syy, mut sx, mut sy) = (0.0, 0.0, 0.0, 0.0, 0.0);
    let (mut bx, mut by, mut bf) = (0.0, 0.0, 0.0);
    for &(x, y) in &xy {
        let q = x * x + y * y;
        sxx += x * x;
        sxy += x * y;
        syy += y * y;
        sx += x;
        sy += y;
        bx -= x * q;
        by -= y * q;
        bf -= q;
    }
    let m = [[sxx, sxy, sx], [sxy, syy, sy], [sx, sy, n as f64]];
    let det3 = |m: [[f64; 3]; 3]| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let det = det3(m);
    if det.abs() < 1e-12 {
        return None;
    }
    let replace = |column: usize, rhs: [f64; 3]| {
        let mut r = m;
        for (row, value) in rhs.iter().enumerate() {
            r[row][column] = *value;
        }
        det3(r) / det
    };
    let rhs = [bx, by, bf];
    let (d, e, f) = (replace(0, rhs), replace(1, rhs), replace(2, rhs));
    let (cx, cy) = (-d / 2.0, -e / 2.0);
    let radius_squared = cx * cx + cy * cy - f;
    if radius_squared <= 1e-18 {
        return None;
    }
    let radius = radius_squared.sqrt();
    if xy.iter().any(|&(x, y)| (((x - cx).powi(2) + (y - cy).powi(2)).sqrt() - radius).abs() > radius * 1e-3) {
        return None;
    }
    let turn: f64 = xy.iter().enumerate().map(|(i, &(x, y))| x * xy[(i + 1) % n].1 - xy[(i + 1) % n].0 * y).sum();
    let direction = if turn >= 0.0 { 1.0 } else { -1.0 };
    let start = (xy[0].1 - cy).atan2(xy[0].0 - cx);
    Some(
        (0..n)
            .map(|i| {
                let theta = start + direction * std::f64::consts::TAU * i as f64 / n as f64;
                add(
                    centroid,
                    add(mul(e1, cx + radius * theta.cos()), mul(e2, cy + radius * theta.sin())),
                )
            })
            .collect(),
    )
}

#[expect(
    clippy::cast_precision_loss,
    reason = "loop indices are bounded by mesh size"
)]
fn smooth_resample(s: &Soup, chain: &[u32], closed: bool) -> Vec<V> {
    if !closed || chain.len() < 3 {
        return resample(s, chain, closed);
    }
    let points: Vec<V> = chain.iter().map(|&v| p(s, v)).collect();
    let n = points.len();
    if let Some(circle) = fit_circle(&points) {
        return circle;
    }
    let steps = n * 24;
    let mut samples = Vec::with_capacity(steps + 1);
    for k in 0..=steps {
        let t = k as f64 / 24.0;
        let seg = (t.floor() as usize).min(n - 1);
        let u = t - seg as f64;
        let p0 = points[(seg + n - 1) % n];
        let p1 = points[seg];
        let p2 = points[(seg + 1) % n];
        let p3 = points[(seg + 2) % n];
        let q = mul(
            add(
                add(mul(p1, 2.0), mul(sub(p2, p0), u)),
                add(
                    mul(
                        add(
                            add(mul(p0, 2.0), mul(p1, -5.0)),
                            add(mul(p2, 4.0), mul(p3, -1.0)),
                        ),
                        u * u,
                    ),
                    mul(
                        add(add(mul(p0, -1.0), mul(p1, 3.0)), add(mul(p2, -3.0), p3)),
                        u * u * u,
                    ),
                ),
            ),
            0.5,
        );
        samples.push(q);
    }
    let mut lengths = vec![0.0];
    for pair in samples.windows(2) {
        lengths.push(lengths.last().copied().unwrap_or(0.0) + distance(pair[0], pair[1]));
    }
    let total = *lengths.last().unwrap_or(&0.0);
    let mut out = Vec::new();
    for j in 0..n {
        let target = total * j as f64 / n as f64;
        let i = (0..steps)
            .find(|&i| lengths[i + 1] >= target)
            .unwrap_or(steps - 1);
        let t = (target - lengths[i]) / (lengths[i + 1] - lengths[i]).max(1e-20);
        out.push(add(mul(samples[i], 1.0 - t), mul(samples[i + 1], t)));
    }
    out
}
fn loop_edit(mut s: Soup, selected: &[[u32; 2]], op: &MeshOp) -> Result<EditedMesh, KernelError> {
    let chains = loops(&s, selected)?;
    match *op {
        MeshOp::SpaceEdgeLoops { factor, linear } => {
            if !factor.is_finite() {
                return Err(KernelError::NonFinite);
            }
            if !(0.0..=1.0).contains(&factor) {
                return Err(KernelError::InvalidArgument("rejected by the kernel"));
            }
            for chain in chains {
                let closed = chain.len() > 2
                    && selected.iter().any(|e| {
                        e.contains(&chain[0]) && e.contains(chain.last().unwrap_or(&chain[0]))
                    });
                // Along the straight segments, or along the smooth curve through the vertices.
                let spaced = if linear {
                    resample(&s, &chain, closed)
                } else {
                    smooth_resample(&s, &chain, closed)
                };
                for (i, &v) in chain.iter().enumerate() {
                    let q = spaced[i];
                    let old = p(&s, v);
                    s.positions[v as usize] = f(add(mul(old, 1.0 - factor), mul(q, factor)));
                }
            }
        }
        MeshOp::RelaxEdgeLoops {
            iterations,
            even_spacing,
            ..
        } => {
            if !(1..=50).contains(&iterations) {
                return Err(KernelError::InvalidArgument("rejected by the kernel"));
            }
            for chain in chains {
                let closed = chain.len() > 2
                    && selected.iter().any(|e| {
                        e.contains(&chain[0]) && e.contains(chain.last().unwrap_or(&chain[0]))
                    });
                for _ in 0..iterations {
                    let old: Vec<V> = chain.iter().map(|&v| p(&s, v)).collect();
                    for i in 0..chain.len() {
                        if !closed && (i == 0 || i + 1 == chain.len()) {
                            continue;
                        }
                        let a = old[(i + chain.len() - 1) % chain.len()];
                        let b = old[(i + 1) % chain.len()];
                        s.positions[chain[i] as usize] = f(mul(add(a, b), 0.5));
                    }
                }
                if even_spacing {
                    let spaced = smooth_resample(&s, &chain, closed);
                    for (i, &v) in chain.iter().enumerate() {
                        s.positions[v as usize] = f(spaced[i]);
                    }
                }
            }
        }
        MeshOp::Circularize {
            factor,
            radius,
            angle,
            flatten,
            regular,
        } => {
            if [factor, radius, angle, flatten]
                .iter()
                .any(|x| !x.is_finite())
            {
                return Err(KernelError::NonFinite);
            }
            if !(0.0..=1.0).contains(&factor) || !(0.0..=1.0).contains(&flatten) || radius < 0.0 {
                return Err(KernelError::InvalidArgument("rejected by the kernel"));
            }
            for chain in chains {
                if chain.len() < 3 {
                    return Err(KernelError::OperatorRefused);
                }
                let pts: Vec<V> = chain.iter().map(|&v| p(&s, v)).collect();
                let mut c = [0.0; 3];
                for &q in &pts {
                    c = add(c, q)
                }
                c = mul(c, 1.0 / pts.len() as f64);
                let mut n = [0.0; 3];
                for i in 0..pts.len() {
                    n = add(n, cross(sub(pts[i], c), sub(pts[(i + 1) % pts.len()], c)));
                }
                n = unit(n);
                if len(n) < 0.5 {
                    return Err(KernelError::OperatorRefused);
                }
                let u = unit(sub(pts[0], c));
                let v = unit(cross(n, u));
                let r = if radius > 0.0 {
                    radius
                } else {
                    pts.iter().map(|&q| distance(q, c)).sum::<f64>() / pts.len() as f64
                };
                let mut angles: Vec<f64> = pts
                    .iter()
                    .map(|&q| dot(sub(q, c), v).atan2(dot(sub(q, c), u)))
                    .collect();
                if regular {
                    angles = (0..pts.len())
                        .map(|i| angle + std::f64::consts::TAU * i as f64 / pts.len() as f64)
                        .collect();
                }
                for (i, &id) in chain.iter().enumerate() {
                    let q = pts[i];
                    let height = dot(sub(q, c), n);
                    let radial = unit(sub(q, c));
                    let circ = add(
                        add(
                            c,
                            mul(add(mul(u, angles[i].cos()), mul(v, angles[i].sin())), r),
                        ),
                        mul(n, height * (1.0 - flatten)),
                    );
                    let flat = add(c, mul(radial, r));
                    let dest = if flatten > 0.0 {
                        add(mul(circ, 1.0 - flatten), mul(flat, flatten))
                    } else {
                        circ
                    };
                    s.positions[id as usize] = f(add(mul(q, 1.0 - factor), mul(dest, factor)));
                }
            }
        }
        _ => return Err(KernelError::OperatorRefused),
    }
    Ok(s.finish())
}

/// Runs native cleanup and edge-loop operations.
#[must_use]
pub fn edit(
    mesh: &PolygonMesh<'_>,
    selection: &MeshSelection<'_>,
    op: &MeshOp,
) -> Option<Result<EditedMesh, KernelError>> {
    let supported = matches!(
        op,
        MeshOp::PlanarFaces { .. }
            | MeshOp::SpaceEdgeLoops { .. }
            | MeshOp::RelaxEdgeLoops { .. }
            | MeshOp::Circularize { .. }
            | MeshOp::Beautify { .. }
    );
    if !supported {
        return None;
    }
    let out = Soup::from_mesh(mesh).and_then(|s| match *op {
        MeshOp::PlanarFaces { iterations, factor } => planar(s, iterations, factor),
        MeshOp::SpaceEdgeLoops { .. } | MeshOp::RelaxEdgeLoops { .. } | MeshOp::Circularize { .. } => {
            loop_edit(s, selection.edges, op)
        }
        MeshOp::Beautify { by_angle } => beautify(s, by_angle),
        _ => Err(KernelError::OperatorRefused),
    });
    let _ = selection;
    Some(out)
}
