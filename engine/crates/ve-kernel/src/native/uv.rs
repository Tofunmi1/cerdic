//! Native UV unwrap (least-squares conformal maps) and island packing.
//!
//! Unwrap cuts the mesh along the seams into islands (a vertex on a seam becomes one UV vertex per fan of faces it
//! still joins), flattens each island with LSCM (a sparse least-squares problem solved by preconditioned conjugate
//! gradients, two far-apart vertices pinned), scales it to its 3D area, turns it to the tightest axis-aligned box and
//! packs the boxes in rows. An island the solver cannot flatten makes the whole call decline (`None`), so the legacy
//! build still serves it.

use std::collections::BTreeMap;

use crate::{KernelError, PolygonMesh, UnwrapOptions, UvLayout, UvPackOptions};

type V3 = [f64; 3];

const MAX_ITERATIONS: usize = 20_000;

/// Face ranges into the corner list, validated.
fn ranges(mesh: &PolygonMesh<'_>) -> Result<Vec<(usize, usize)>, KernelError> {
    let mut out = Vec::with_capacity(mesh.face_sizes.len());
    let mut at = 0_usize;
    for &n in mesh.face_sizes {
        let n = n as usize;
        if n < 3 || at.checked_add(n).is_none_or(|end| end > mesh.corners.len()) {
            return Err(KernelError::InvalidArgument(
                "a face needs at least three corners that exist",
            ));
        }
        out.push((at, at + n));
        at += n;
    }
    if at != mesh.corners.len() {
        return Err(KernelError::InvalidArgument(
            "face sizes do not cover the corners",
        ));
    }
    if mesh.positions.iter().flatten().any(|v| !v.is_finite()) {
        return Err(KernelError::NonFinite);
    }
    if mesh.positions.len() > 1_000_000 || mesh.face_sizes.len() > 1_000_000 || at > 4_000_000 {
        return Err(KernelError::TooLarge);
    }
    if mesh
        .corners
        .iter()
        .any(|&v| v as usize >= mesh.positions.len())
    {
        return Err(KernelError::OutOfBounds);
    }
    Ok(out)
}

fn valid_margin(margin: f32) -> bool {
    margin.is_finite() && (0.0..=0.5).contains(&margin)
}

struct DisjointSets(Vec<usize>);

impl DisjointSets {
    fn new(n: usize) -> Self {
        Self((0..n).collect())
    }
    fn find(&mut self, mut x: usize) -> usize {
        while self.0[x] != x {
            self.0[x] = self.0[self.0[x]];
            x = self.0[x];
        }
        x
    }
    fn union(&mut self, a: usize, b: usize) {
        let (a, b) = (self.find(a), self.find(b));
        if a != b {
            self.0[a.max(b)] = a.min(b);
        }
    }
}

/// An island's flattened corners, ready to place.
struct Island {
    /// Corner indices, in mesh corner order.
    corners: Vec<usize>,
    /// UV of each entry of `corners`.
    uvs: Vec<[f64; 2]>,
}

impl Island {
    fn bounds(&self) -> ([f64; 2], [f64; 2]) {
        let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        for p in &self.uvs {
            for d in 0..2 {
                lo[d] = lo[d].min(p[d]);
                hi[d] = hi[d].max(p[d]);
            }
        }
        (lo, hi)
    }
}

/// Where a packed island's lower-left corner goes.
struct Placement {
    x: f64,
    y: f64,
}

/// Rows of islands inside the unit square, tallest first, or `None` if they do not fit at this scale.
fn shelf(sizes: &[[f64; 2]], scale: f64, margin: f64) -> Option<Vec<Placement>> {
    let mut order: Vec<usize> = (0..sizes.len()).collect();
    order.sort_by(|&a, &b| sizes[b][1].total_cmp(&sizes[a][1]).then(a.cmp(&b)));
    let limit = 1.0 - margin;
    let mut placements: Vec<Option<Placement>> = sizes.iter().map(|_| None).collect();
    let (mut x, mut y, mut row) = (margin, margin, 0.0_f64);
    for &i in &order {
        let (w, h) = (sizes[i][0] * scale, sizes[i][1] * scale);
        if x + w > limit + 1e-12 && x > margin {
            x = margin;
            y += row + margin;
            row = 0.0;
        }
        if x + w > limit + 1e-12 || y + h > limit + 1e-12 {
            return None;
        }
        placements[i] = Some(Placement { x, y });
        x += w + margin;
        row = row.max(h);
    }
    placements.into_iter().collect()
}

/// Packs islands (already in UV units) into the unit square and returns one UV per mesh corner.
fn pack_islands(
    islands: &[Island],
    corner_count: usize,
    rotate: bool,
    keep_density: bool,
    margin: f32,
) -> Option<Vec<[f32; 2]>> {
    let margin = f64::from(margin);
    // With rotation allowed, turn islands a quarter so they are wider than tall.
    let mut turned = vec![false; islands.len()];
    let mut sizes = Vec::with_capacity(islands.len());
    for (k, island) in islands.iter().enumerate() {
        let (lo, hi) = island.bounds();
        let (w, h) = ((hi[0] - lo[0]).max(1e-9), (hi[1] - lo[1]).max(1e-9));
        if rotate && h > w {
            turned[k] = true;
            sizes.push([h, w]);
        } else {
            sizes.push([w, h]);
        }
    }
    let widest = sizes.iter().map(|s| s[0].max(s[1])).fold(0.0_f64, f64::max);
    if widest <= 0.0 || 1.0 - 2.0 * margin <= 0.0 {
        return None;
    }
    let mut high = (1.0 - 2.0 * margin) / widest;
    if keep_density {
        high = high.min(1.0);
    }
    let mut scale = if shelf(&sizes, high, margin).is_some() {
        high
    } else {
        let (mut low, mut top) = (0.0_f64, high);
        for _ in 0..50 {
            let mid = (low + top) / 2.0;
            if shelf(&sizes, mid, margin).is_some() {
                low = mid;
            } else {
                top = mid;
            }
        }
        low
    };
    if scale <= 0.0 {
        scale = high * 1e-3;
    }
    let placements = shelf(&sizes, scale, margin)?;
    let mut out = vec![[0.0_f32; 2]; corner_count];
    for (k, island) in islands.iter().enumerate() {
        let (lo, hi) = island.bounds();
        let place = &placements[k];
        for (&corner, p) in island.corners.iter().zip(&island.uvs) {
            // A quarter turn keeps the island's handedness: (u, v) -> (hi_v - v, u - lo_u).
            let (u, v) = if turned[k] {
                (hi[1] - p[1], p[0] - lo[0])
            } else {
                (p[0] - lo[0], p[1] - lo[1])
            };
            #[expect(clippy::cast_possible_truncation, reason = "UVs are stored as f32")]
            let uv = [
                (place.x + u * scale).clamp(0.0, 1.0) as f32,
                (place.y + v * scale).clamp(0.0, 1.0) as f32,
            ];
            out[corner] = uv;
        }
    }
    Some(out)
}

/// For each corner `i` of face `f` going to the next corner, the corners of the neighbour across that edge that sit at
/// the same two vertices.
fn edge_users(
    mesh: &PolygonMesh<'_>,
    faces: &[(usize, usize)],
) -> BTreeMap<(u32, u32), Vec<(usize, usize)>> {
    let mut edges: BTreeMap<(u32, u32), Vec<(usize, usize)>> = BTreeMap::new();
    for (f, &(start, end)) in faces.iter().enumerate() {
        for i in start..end {
            let next = if i + 1 == end { start } else { i + 1 };
            let (a, b) = (mesh.corners[i], mesh.corners[next]);
            edges.entry((a.min(b), a.max(b))).or_default().push((f, i));
        }
    }
    edges
}

fn next_corner(faces: &[(usize, usize)], face: usize, corner: usize) -> usize {
    let (start, end) = faces[face];
    if corner + 1 == end { start } else { corner + 1 }
}

/// Packs islands (faces joined across edges whose UVs agree on both sides) into the unit square.
#[must_use]
pub fn pack_uvs(
    mesh: &PolygonMesh<'_>,
    uvs: &[[f32; 2]],
    options: &UvPackOptions,
) -> Option<Result<Vec<[f32; 2]>, KernelError>> {
    let result = (|| {
        let faces = ranges(mesh)?;
        if uvs.len() != mesh.corners.len() {
            return Err(KernelError::InvalidArgument(
                "one UV value is required per mesh corner",
            ));
        }
        if !valid_margin(options.margin) {
            return Err(KernelError::InvalidArgument("invalid margin"));
        }
        if uvs.iter().flatten().any(|v| !v.is_finite()) {
            return Err(KernelError::NonFinite);
        }
        let mut sets = DisjointSets::new(faces.len());
        for users in edge_users(mesh, &faces).values() {
            if let [(f, i), (g, j)] = users[..] {
                let (fa, fb) = (i, next_corner(&faces, f, i));
                let (ga, gb) = (j, next_corner(&faces, g, j));
                let (ga, gb) = if mesh.corners[fa] == mesh.corners[ga] {
                    (ga, gb)
                } else {
                    (gb, ga)
                };
                if uvs[fa] == uvs[ga] && uvs[fb] == uvs[gb] {
                    sets.union(f, g);
                }
            }
        }
        let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for f in 0..faces.len() {
            let root = sets.find(f);
            groups.entry(root).or_default().push(f);
        }
        let islands: Vec<Island> = groups
            .values()
            .map(|group| {
                let corners: Vec<usize> =
                    group.iter().flat_map(|&f| faces[f].0..faces[f].1).collect();
                let uvs = corners.iter().map(|&c| uvs[c].map(f64::from)).collect();
                Island { corners, uvs }
            })
            .collect();
        pack_islands(
            &islands,
            mesh.corners.len(),
            options.rotate,
            options.keep_density,
            options.margin,
        )
        .ok_or(KernelError::InvalidArgument(
            "the islands do not fit the unit square",
        ))
    })();
    Some(result)
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
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
fn length(a: V3) -> f64 {
    dot(a, a).sqrt()
}

/// One sparse row of the normal equations.
type Row = BTreeMap<usize, f64>;

/// Solves `A x = b` for a symmetric positive semi-definite sparse `A` by Jacobi-preconditioned conjugate gradients.
fn conjugate_gradient(rows: &[Row], b: &[f64]) -> Option<Vec<f64>> {
    let n = b.len();
    let diagonal: Vec<f64> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| r.get(&i).copied().unwrap_or(0.0))
        .collect();
    if diagonal.iter().any(|&d| d <= 1e-300) {
        return None;
    }
    let multiply = |x: &[f64]| -> Vec<f64> {
        rows.iter()
            .map(|r| r.iter().map(|(&j, &a)| a * x[j]).sum())
            .collect()
    };
    let dotv = |a: &[f64], b: &[f64]| -> f64 { a.iter().zip(b).map(|(x, y)| x * y).sum() };
    let mut x = vec![0.0; n];
    let mut r = b.to_vec();
    let target = 1e-11 * dotv(b, b).sqrt().max(1e-30);
    if dotv(&r, &r).sqrt() <= target {
        return Some(x);
    }
    let mut z: Vec<f64> = r.iter().zip(&diagonal).map(|(r, d)| r / d).collect();
    let mut p = z.clone();
    let mut rz = dotv(&r, &z);
    for _ in 0..MAX_ITERATIONS.min(60 * n + 200) {
        let ap = multiply(&p);
        let denominator = dotv(&p, &ap);
        if denominator <= 1e-300 {
            return None;
        }
        let alpha = rz / denominator;
        for i in 0..n {
            x[i] += alpha * p[i];
            r[i] -= alpha * ap[i];
        }
        if dotv(&r, &r).sqrt() <= target {
            return x.iter().all(|v| v.is_finite()).then_some(x);
        }
        z = r.iter().zip(&diagonal).map(|(r, d)| r / d).collect();
        let next = dotv(&r, &z);
        let beta = next / rz;
        for i in 0..n {
            p[i] = z[i] + beta * p[i];
        }
        rz = next;
    }
    None
}

/// Flattens one island: `triangles` index its own vertex list, `positions` are those vertices in 3D.
/// Returns a UV per island vertex, scaled so the total area equals the 3D area, or `None` if it cannot be flattened.
fn lscm(positions: &[V3], triangles: &[[usize; 3]]) -> Option<Vec<[f64; 2]>> {
    let n = positions.len();
    if n < 3 || triangles.is_empty() {
        return None;
    }
    // Pin the two vertices that are farthest apart (found with two sweeps).
    #[expect(
        clippy::cast_precision_loss,
        reason = "vertex counts are far below 2^52"
    )]
    let centre = positions
        .iter()
        .fold([0.0; 3], |a, p| [a[0] + p[0], a[1] + p[1], a[2] + p[2]])
        .map(|c| c / n as f64);
    let far = |from: V3| {
        (0..n)
            .max_by(|&i, &j| {
                length(sub(positions[i], from)).total_cmp(&length(sub(positions[j], from)))
            })
            .unwrap_or(0)
    };
    let a = far(centre);
    let b = far(positions[a]);
    let span = length(sub(positions[a], positions[b]));
    if a == b || span < 1e-12 {
        return None;
    }
    let pinned: BTreeMap<usize, f64> = BTreeMap::from([
        (2 * a, 0.0),
        (2 * a + 1, 0.0),
        (2 * b, span),
        (2 * b + 1, 0.0),
    ]);
    let mut index = vec![usize::MAX; 2 * n];
    let mut free = 0_usize;
    for (var, slot) in index.iter_mut().enumerate() {
        if !pinned.contains_key(&var) {
            *slot = free;
            free += 1;
        }
    }
    let mut rows: Vec<Row> = vec![Row::new(); free];
    let mut rhs = vec![0.0; free];
    let mut area_3d = 0.0;
    for tri in triangles {
        let p = [positions[tri[0]], positions[tri[1]], positions[tri[2]]];
        let e1 = sub(p[1], p[0]);
        let l = length(e1);
        let q = sub(p[2], p[0]);
        let twice = length(cross(e1, q));
        if l < 1e-12 || twice < 1e-12 {
            continue;
        }
        area_3d += twice / 2.0;
        let ex = [e1[0] / l, e1[1] / l, e1[2] / l];
        let x = dot(q, ex);
        let y = (dot(q, q) - x * x).max(0.0).sqrt();
        // Local corners (0,0), (l,0), (x,y). The conformal condition is two linear rows per triangle.
        let local = [[0.0, 0.0], [l, 0.0], [x, y]];
        let weight = 1.0 / twice.sqrt() / std::f64::consts::SQRT_2;
        let mut real = [(0_usize, 0.0_f64); 6];
        let mut imaginary = [(0_usize, 0.0_f64); 6];
        for j in 0..3 {
            let (j1, j2) = ((j + 1) % 3, (j + 2) % 3);
            let bj = (local[j1][1] - local[j2][1]) * weight;
            let cj = (local[j2][0] - local[j1][0]) * weight;
            let vertex = tri[j];
            real[2 * j] = (2 * vertex, bj);
            real[2 * j + 1] = (2 * vertex + 1, -cj);
            imaginary[2 * j] = (2 * vertex, cj);
            imaginary[2 * j + 1] = (2 * vertex + 1, bj);
        }
        for row in [&real, &imaginary] {
            for &(vi, ci) in row {
                if pinned.contains_key(&vi) {
                    continue;
                }
                let fi = index[vi];
                for &(vj, cj) in row {
                    if let Some(&value) = pinned.get(&vj) {
                        rhs[fi] -= ci * cj * value;
                    } else {
                        *rows[fi].entry(index[vj]).or_insert(0.0) += ci * cj;
                    }
                }
            }
        }
    }
    if area_3d < 1e-12 {
        return None;
    }
    let solution = conjugate_gradient(&rows, &rhs)?;
    let mut uv = vec![[0.0_f64; 2]; n];
    for (vertex, p) in uv.iter_mut().enumerate() {
        for d in 0..2 {
            let var = 2 * vertex + d;
            p[d] = pinned
                .get(&var)
                .copied()
                .unwrap_or_else(|| solution[index[var]]);
        }
    }
    // Scale so the island keeps its 3D area.
    let mut area_uv = 0.0;
    for tri in triangles {
        let (p, q, r) = (uv[tri[0]], uv[tri[1]], uv[tri[2]]);
        area_uv += ((q[0] - p[0]) * (r[1] - p[1]) - (r[0] - p[0]) * (q[1] - p[1])).abs() / 2.0;
    }
    if area_uv.is_nan() || area_uv <= 1e-12 {
        return None;
    }
    let scale = (area_3d / area_uv).sqrt();
    for p in &mut uv {
        p[0] *= scale;
        p[1] *= scale;
    }
    Some(uv)
}

/// Turns the points so their axis-aligned box has the least area, the longer side along u, and moves its corner to the origin.
fn tighten(points: &mut [[f64; 2]]) {
    let mut best: Option<(f64, f64)> = None;
    for step in 0..90 {
        let angle = std::f64::consts::FRAC_PI_2 * f64::from(step) / 90.0;
        let (s, c) = angle.sin_cos();
        let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        for p in points.iter() {
            let r = [c * p[0] - s * p[1], s * p[0] + c * p[1]];
            for d in 0..2 {
                lo[d] = lo[d].min(r[d]);
                hi[d] = hi[d].max(r[d]);
            }
        }
        let area = (hi[0] - lo[0]) * (hi[1] - lo[1]);
        if best.is_none_or(|(a, _)| area < a - 1e-12) {
            best = Some((area, angle));
        }
    }
    let (s, c) = best.map_or((0.0, 1.0), |(_, angle)| angle.sin_cos());
    for p in points.iter_mut() {
        *p = [c * p[0] - s * p[1], s * p[0] + c * p[1]];
    }
    let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    for p in points.iter() {
        for d in 0..2 {
            lo[d] = lo[d].min(p[d]);
            hi[d] = hi[d].max(p[d]);
        }
    }
    let width = hi[0] - lo[0];
    let swap = hi[1] - lo[1] > width;
    for p in points.iter_mut() {
        let q = [p[0] - lo[0], p[1] - lo[1]];
        // A quarter turn (not a reflection), so the island keeps its handedness.
        *p = if swap { [q[1], width - q[0]] } else { q };
    }
}

/// Cuts along the seams, flattens every island and (optionally) packs them.
#[must_use]
pub fn unwrap_uvs(
    mesh: &PolygonMesh<'_>,
    seams: &[[u32; 2]],
    options: &UnwrapOptions,
) -> Option<Result<UvLayout, KernelError>> {
    let faces = match ranges(mesh) {
        Ok(faces) => faces,
        Err(error) => return Some(Err(error)),
    };
    if !valid_margin(options.margin) {
        return Some(Err(KernelError::InvalidArgument("invalid margin")));
    }
    let edges = edge_users(mesh, &faces);
    let mut seam_set = std::collections::BTreeSet::new();
    for pair in seams {
        if pair.iter().any(|&v| v as usize >= mesh.positions.len()) {
            return Some(Err(KernelError::OutOfBounds));
        }
        let key = (pair[0].min(pair[1]), pair[0].max(pair[1]));
        if !edges.contains_key(&key) {
            return Some(Err(KernelError::InvalidArgument("rejected by the kernel")));
        }
        seam_set.insert(key);
    }
    if edges.values().any(|users| users.len() > 2) {
        return None;
    }
    // Faces joined across plain edges form islands; their corners at a shared vertex are one UV vertex.
    let mut face_sets = DisjointSets::new(faces.len());
    let mut corner_sets = DisjointSets::new(mesh.corners.len());
    for (key, users) in &edges {
        if seam_set.contains(key) {
            continue;
        }
        if let [(f, i), (g, j)] = users[..] {
            face_sets.union(f, g);
            let (fa, fb) = (i, next_corner(&faces, f, i));
            let (ga, gb) = (j, next_corner(&faces, g, j));
            let (ga, gb) = if mesh.corners[fa] == mesh.corners[ga] {
                (ga, gb)
            } else {
                (gb, ga)
            };
            corner_sets.union(fa, ga);
            corner_sets.union(fb, gb);
        }
    }
    let mut islands_of: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for f in 0..faces.len() {
        let root = face_sets.find(f);
        islands_of.entry(root).or_default().push(f);
    }
    let mut islands: Vec<Island> = Vec::with_capacity(islands_of.len());
    let mut failed = 0_u32;
    for group in islands_of.values() {
        let mut local: BTreeMap<usize, usize> = BTreeMap::new();
        let mut positions: Vec<V3> = Vec::new();
        let mut triangles: Vec<[usize; 3]> = Vec::new();
        let mut corners = Vec::new();
        let mut corner_vertex = Vec::new();
        for &f in group {
            let (start, end) = faces[f];
            let mut ids = Vec::with_capacity(end - start);
            for c in start..end {
                let root = corner_sets.find(c);
                let next = local.len();
                let id = *local.entry(root).or_insert_with(|| {
                    positions.push(mesh.positions[mesh.corners[c] as usize].map(f64::from));
                    next
                });
                ids.push(id);
                corners.push(c);
                corner_vertex.push(id);
            }
            for k in 1..ids.len() - 1 {
                triangles.push([ids[0], ids[k], ids[k + 1]]);
            }
        }
        // An island the solver cannot flatten (a closed piece with no seam, a degenerate one) is counted and left at
        // (0, 0), as the result's documentation says; the rest of the mesh is still unwrapped.
        let Some(mut uv) = lscm(&positions, &triangles) else {
            failed += 1;
            continue;
        };
        tighten(&mut uv);
        let uvs = corner_vertex.iter().map(|&v| uv[v]).collect();
        islands.push(Island { corners, uvs });
    }
    let corner_count = mesh.corners.len();
    let out = if options.pack && !islands.is_empty() {
        pack_islands(
            &islands,
            corner_count,
            options.rotate,
            false,
            options.margin,
        )?
    } else {
        let mut out = vec![[0.0_f32; 2]; corner_count];
        for island in &islands {
            for (&c, p) in island.corners.iter().zip(&island.uvs) {
                #[expect(clippy::cast_possible_truncation, reason = "UVs are stored as f32")]
                {
                    out[c] = [p[0] as f32, p[1] as f32];
                }
            }
        }
        out
    };
    Some(Ok(UvLayout {
        uvs: out,
        failed_islands: failed,
    }))
}
