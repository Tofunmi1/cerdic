//! Plane fitting and triangle clipping for clean garment boundaries.
use std::collections::BTreeMap;

use ve_kernel::Mesh;

#[derive(Clone, Copy, Debug)]
struct Plane {
    point: [f32; 3],
    normal: [f32; 3],
    center: [f32; 3],
    radius: f32,
}

pub(super) struct Clipped {
    pub mesh: Mesh,
    pub uvs: Vec<[f32; 2]>,
    pub nearest: Vec<usize>,
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn boundary_loops(mesh: &Mesh) -> Vec<Vec<usize>> {
    let mut edges = BTreeMap::<(usize, usize), usize>::new();
    for tri in mesh.indices.chunks_exact(3) {
        for (a, b) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
            let (a, b) = (a as usize, b as usize);
            *edges.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    let mut next = BTreeMap::<usize, Vec<usize>>::new();
    for ((a, b), count) in edges {
        if count == 1 {
            next.entry(a).or_default().push(b);
            next.entry(b).or_default().push(a);
        }
    }
    let mut used = std::collections::BTreeSet::new();
    let mut loops = Vec::new();
    for (&start, neighbors) in &next {
        for &first in neighbors {
            if used.contains(&(start.min(first), start.max(first))) {
                continue;
            }
            let mut chain = vec![start];
            let (mut prev, mut at) = (start, first);
            used.insert((prev.min(at), prev.max(at)));
            while at != start {
                chain.push(at);
                let Some(follow) = next.get(&at).and_then(|ns| {
                    ns.iter()
                        .copied()
                        .find(|n| *n != prev && !used.contains(&(at.min(*n), at.max(*n))))
                }) else {
                    break;
                };
                prev = at;
                at = follow;
                used.insert((prev.min(at), prev.max(at)));
            }
            if at == start && chain.len() >= 3 {
                loops.push(chain);
            }
        }
    }
    loops
}

fn fit(points: &[[f32; 3]]) -> Option<Plane> {
    if points.len() < 3 {
        return None;
    }
    let count = points.len() as f32;
    let center = std::array::from_fn(|i| points.iter().map(|p| p[i]).sum::<f32>() / count);
    let mut a = [[0.0_f64; 3]; 3];
    for p in points {
        let d = sub(*p, center).map(f64::from);
        for i in 0..3 {
            for j in 0..3 {
                a[i][j] += d[i] * d[j];
            }
        }
    }
    let mut v = [[1.0_f64, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..16 {
        let (mut p, mut q) = (0, 1);
        for (i, row) in a.iter().enumerate() {
            for (j, value) in row.iter().enumerate().skip(i + 1) {
                if value.abs() > a[p][q].abs() {
                    p = i;
                    q = j;
                }
            }
        }
        if a[p][q].abs() < 1e-12 {
            break;
        }
        let tau = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
        let t = tau.signum() / (tau.abs() + (1.0 + tau * tau).sqrt());
        let c = 1.0 / (1.0 + t * t).sqrt();
        let s = t * c;
        let (app, aqq, apq) = (a[p][p], a[q][q], a[p][q]);
        a[p][p] = app - t * apq;
        a[q][q] = aqq + t * apq;
        a[p][q] = 0.0;
        a[q][p] = 0.0;
        for k in 0..3 {
            if k == p || k == q {
                continue;
            }
            let (akp, akq) = (a[k][p], a[k][q]);
            a[k][p] = c * akp - s * akq;
            a[p][k] = a[k][p];
            a[k][q] = s * akp + c * akq;
            a[q][k] = a[k][q];
        }
        for row in &mut v {
            let (x, y) = (row[p], row[q]);
            row[p] = c * x - s * y;
            row[q] = s * x + c * y;
        }
    }
    let axis = (0..3).min_by(|i, j| a[*i][*i].total_cmp(&a[*j][*j]))?;
    let mut normal = [v[0][axis] as f32, v[1][axis] as f32, v[2][axis] as f32];
    let length = dot(normal, normal).sqrt();
    if !length.is_finite() || length < 1e-8 {
        return None;
    }
    normal = normal.map(|x| x / length);
    let mut projections: Vec<f32> = points.iter().map(|p| dot(*p, normal)).collect();
    projections.sort_by(f32::total_cmp);
    let median = projections[projections.len() / 2];
    let point = normal.map(|x| x * median);
    let radius = points
        .iter()
        .map(|p| {
            let d = sub(*p, center);
            let across = sub(d, normal.map(|n| n * dot(d, normal)));
            dot(across, across).sqrt()
        })
        .fold(0.0_f32, f32::max);
    Some(Plane {
        point,
        normal,
        center,
        radius: radius * 1.35 + 0.015,
    })
}

fn axis_plane(points: &[[f32; 3]], axis: usize, inside: f32) -> Option<Plane> {
    if points.len() < 3 {
        return None;
    }
    let mut projections: Vec<f32> = points.iter().map(|p| p[axis]).collect();
    projections.sort_by(f32::total_cmp);
    let mut center = [0.0_f32; 3];
    for point in points {
        for i in 0..3 {
            center[i] += point[i] / points.len() as f32;
        }
    }
    let mut normal = [0.0_f32; 3];
    normal[axis] = 1.0;
    let radius = points
        .iter()
        .map(|p| {
            let d = sub(*p, center);
            let across = sub(d, normal.map(|n| n * dot(d, normal)));
            dot(across, across).sqrt()
        })
        .fold(0.0_f32, f32::max);
    let mut point = center;
    // The cut goes through the middle of the ragged edge: cloth beyond it is clipped away, and the notches on the cloth side are closed by `snap_to_planes`.
    let _ = inside;
    point[axis] = projections[projections.len() / 2];
    Some(Plane {
        point,
        normal,
        center,
        radius: radius * 5.0 + 0.02,
    })
}

#[derive(Clone, Copy)]
struct Vertex {
    p: [f32; 3],
    uv: [f32; 2],
    source: usize,
}

fn clip_one(
    mesh: Mesh,
    uvs: Vec<[f32; 2]>,
    nearest: Vec<usize>,
    plane: Plane,
    inside: [f32; 3],
) -> Clipped {
    let sign = if dot(sub(inside, plane.point), plane.normal) >= 0.0 {
        1.0
    } else {
        -1.0
    };
    let mut out = Clipped {
        mesh: Mesh::default(),
        uvs: Vec::new(),
        nearest: Vec::new(),
    };
    for (t, tri) in mesh.indices.chunks_exact(3).enumerate() {
        let centroid = tri.iter().fold([0.0_f32; 3], |mut sum, index| {
            for axis in 0..3 {
                sum[axis] += mesh.positions[*index as usize][axis] / 3.0;
            }
            sum
        });
        let relative = sub(centroid, plane.center);
        let across = sub(
            relative,
            plane.normal.map(|n| n * dot(relative, plane.normal)),
        );
        if dot(across, across) > plane.radius * plane.radius {
            for (corner, index) in tri.iter().enumerate() {
                let vertex = *index as usize;
                let base = out.mesh.positions.len() as u32;
                out.mesh.positions.push(mesh.positions[vertex]);
                out.mesh.indices.push(base);
                out.uvs
                    .push(uvs.get(t * 3 + corner).copied().unwrap_or([0.0; 2]));
                out.nearest.push(nearest[vertex]);
            }
            continue;
        }
        let poly: Vec<Vertex> = tri
            .iter()
            .enumerate()
            .map(|(c, i)| {
                let i = *i as usize;
                Vertex {
                    p: mesh.positions[i],
                    uv: uvs.get(t * 3 + c).copied().unwrap_or([0.0; 2]),
                    source: nearest[i],
                }
            })
            .collect();
        let mut clipped = Vec::new();
        for i in 0..poly.len() {
            let a = poly[i];
            let b = poly[(i + 1) % poly.len()];
            let da = dot(sub(a.p, plane.point), plane.normal) * sign;
            let db = dot(sub(b.p, plane.point), plane.normal) * sign;
            if da >= -1e-7 {
                clipped.push(a);
            }
            if (da < 0.0 && db > 0.0) || (da > 0.0 && db < 0.0) {
                let f = da / (da - db);
                clipped.push(Vertex {
                    p: std::array::from_fn(|k| a.p[k] + (b.p[k] - a.p[k]) * f),
                    uv: std::array::from_fn(|k| a.uv[k] + (b.uv[k] - a.uv[k]) * f),
                    source: if f <= 0.5 { a.source } else { b.source },
                });
            }
        }
        if clipped.len() < 3 {
            continue;
        }
        // A fan of triangles with a vertex of its own per corner (like the triangles kept whole), so the uvs stay one per corner.
        for i in 1..clipped.len() - 1 {
            for v in [clipped[0], clipped[i], clipped[i + 1]] {
                out.mesh.indices.push(out.mesh.positions.len() as u32);
                out.mesh.positions.push(v.p);
                out.uvs.push(v.uv);
                out.nearest.push(v.source);
            }
        }
    }
    out
}

pub(super) fn apply(mesh: Mesh, uvs: Vec<[f32; 2]>, nearest: Vec<usize>) -> Clipped {
    let loops = boundary_loops(&mesh);
    let boundary: Vec<[f32; 3]> = loops.iter().flatten().map(|i| mesh.positions[*i]).collect();
    let waist: Vec<[f32; 3]> = boundary
        .iter()
        .copied()
        .filter(|p| p[0].abs() < 0.3 && (0.7..1.15).contains(&p[1]))
        .collect();
    let inside = mesh
        .positions
        .iter()
        .fold([0.0; 3], |mut sum, p| {
            for k in 0..3 {
                sum[k] += p[k];
            }
            sum
        })
        .map(|x| x / mesh.positions.len().max(1) as f32);
    let mut planes = Vec::new();
    if let Some(plane) = axis_plane(&waist, 1, inside[1]) {
        planes.push(plane);
    }
    for sign in [-1.0_f32, 1.0] {
        let cuff: Vec<[f32; 3]> = boundary
            .iter()
            .copied()
            .filter(|p| p[0] * sign > 0.3 && (0.85..1.4).contains(&p[1]))
            .collect();
        if let Some(plane) = axis_plane(&cuff, 0, inside[0]) {
            planes.push(plane);
        }
    }
    if planes.is_empty() {
        planes = loops
            .iter()
            .filter_map(|chain| fit(&chain.iter().map(|i| mesh.positions[*i]).collect::<Vec<_>>()))
            .collect();
    }
    let mut result = Clipped { mesh, uvs, nearest };
    for plane in &planes {
        result = clip_one(result.mesh, result.uvs, result.nearest, *plane, inside);
    }
    snap_to_planes(&mut result.mesh, &planes);
    result
}

/// How far from a cut plane a boundary vertex may be and still be pulled onto it (the notches the skin regions leave in an edge).
const SNAP: f32 = 0.22;

/// Pulls the vertices of the open edge that lie within `SNAP` of a cut plane onto it, so the notches left in the cloth close and the hem is flat.
fn snap_to_planes(mesh: &mut Mesh, planes: &[Plane]) {
    let on_edge: Vec<usize> = boundary_loops(mesh).into_iter().flatten().collect();
    for plane in planes {
        for &i in &on_edge {
            let p = mesh.positions[i];
            let distance = dot(sub(p, plane.point), plane.normal);
            let across = sub(
                sub(p, plane.center),
                plane
                    .normal
                    .map(|n| n * dot(sub(p, plane.center), plane.normal)),
            );
            if distance.abs() <= SNAP && dot(across, across).sqrt() <= plane.radius {
                mesh.positions[i] = sub(p, plane.normal.map(|n| n * distance));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plane_fit_finds_horizontal_cut() {
        let points = [
            [-1.0, -1.0, 0.0],
            [1.0, -1.0, 0.0],
            [1.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0],
            [0.0, 0.0, 0.01],
        ];
        let plane = fit(&points).expect("plane");
        assert!(plane.normal[2].abs() > 0.99);
        assert!(
            points
                .iter()
                .map(|p| dot(*p, plane.normal))
                .fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(x), b.max(x)))
                .1
                .abs()
                < 0.02
        );
    }

    #[test]
    fn clip_interpolates_a_crossing_triangle() {
        let mesh = Mesh {
            positions: vec![[-1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]],
            indices: vec![0, 1, 2],
        };
        let plane = Plane {
            point: [0.0; 3],
            normal: [1.0, 0.0, 0.0],
            center: [0.0; 3],
            radius: 10.0,
        };
        let clipped = clip_one(
            mesh,
            vec![[0.0; 2]; 3],
            vec![0, 1, 2],
            plane,
            [1.0, 0.0, 0.0],
        );
        assert_eq!(clipped.mesh.triangle_count(), 2);
        assert!(clipped.mesh.positions.iter().all(|p| p[0] >= -1e-6));
        assert_eq!(clipped.uvs.len(), clipped.mesh.indices.len());
    }
}
