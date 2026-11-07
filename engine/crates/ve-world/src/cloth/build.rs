//! Solvers built from shapes: cloth from a triangle mesh, rope from points, a balloon from a closed mesh.

use std::collections::BTreeMap;

use super::{MAX_PARTICLES, Solver, V3, cross, dot, sub};

fn length(a: V3, b: V3) -> f32 {
    let d = sub(a, b);
    dot(d, d).sqrt()
}

/// Cloth: a distance constraint on every edge and one across every interior edge (between the two vertices opposite it) that resists
/// folding. Mass is `density` (kilograms per square metre) times each vertex's third of its triangles' area. `None` past the particle cap.
#[must_use]
pub fn cloth_from_mesh(
    positions: &[V3],
    triangles: &[[u32; 3]],
    density: f32,
    stretch_compliance: f32,
    bend_compliance: f32,
) -> Option<Solver> {
    if positions.len() > MAX_PARTICLES
        || triangles
            .iter()
            .flatten()
            .any(|&i| i as usize >= positions.len())
    {
        return None;
    }
    let mut mass = vec![0.0_f32; positions.len()];
    let mut edges: BTreeMap<(u32, u32), Vec<u32>> = BTreeMap::new();
    for t in triangles {
        let [a, b, c] = t.map(|i| positions[i as usize]);
        let area = 0.5 * dot(cross(sub(b, a), sub(c, a)), cross(sub(b, a), sub(c, a))).sqrt();
        for (k, &v) in t.iter().enumerate() {
            mass[v as usize] += density * area / 3.0;
            let (p, q, opposite) = (v, t[(k + 1) % 3], t[(k + 2) % 3]);
            edges
                .entry((p.min(q), p.max(q)))
                .or_default()
                .push(opposite);
        }
    }
    let inv_mass = mass
        .iter()
        .map(|&m| if m > 1e-12 { 1.0 / m } else { 0.0 })
        .collect();
    let mut solver = Solver::new(positions.to_vec(), inv_mass);
    for (&(a, b), opposite) in &edges {
        solver.add_distance(
            a,
            b,
            length(positions[a as usize], positions[b as usize]),
            stretch_compliance,
        );
        if let [c, d] = opposite[..] {
            solver.add_distance(
                c,
                d,
                length(positions[c as usize], positions[d as usize]),
                bend_compliance,
            );
        }
    }
    solver.set_triangles(triangles.to_vec());
    Some(solver)
}

/// A rope or strand through `points`: a distance constraint between neighbours and one skipping a point, which resists sharp kinks.
/// `mass_per_metre` is in kilograms per metre. Pin the root with [`Solver::pin`].
#[must_use]
pub fn rope(
    points: &[V3],
    mass_per_metre: f32,
    stretch_compliance: f32,
    bend_compliance: f32,
) -> Option<Solver> {
    if points.len() < 2 || points.len() > MAX_PARTICLES {
        return None;
    }
    let mut mass = vec![0.0_f32; points.len()];
    for k in 0..points.len() - 1 {
        let half = 0.5 * mass_per_metre * length(points[k], points[k + 1]);
        mass[k] += half;
        mass[k + 1] += half;
    }
    let inv_mass = mass
        .iter()
        .map(|&m| if m > 1e-12 { 1.0 / m } else { 0.0 })
        .collect();
    let mut solver = Solver::new(points.to_vec(), inv_mass);
    for k in 0..points.len() - 1 {
        solver.add_distance(
            k as u32,
            k as u32 + 1,
            length(points[k], points[k + 1]),
            stretch_compliance,
        );
    }
    for k in 0..points.len().saturating_sub(2) {
        solver.add_distance(
            k as u32,
            k as u32 + 2,
            length(points[k], points[k + 2]),
            bend_compliance,
        );
    }
    Some(solver)
}

/// A soft body: a closed triangle mesh with a distance constraint on every edge and a volume held at `pressure` times its present volume.
#[must_use]
pub fn balloon(
    positions: &[V3],
    triangles: &[[u32; 3]],
    density: f32,
    pressure: f32,
    stretch_compliance: f32,
    volume_compliance: f32,
) -> Option<Solver> {
    let mut solver = cloth_from_mesh(
        positions,
        triangles,
        density,
        stretch_compliance,
        stretch_compliance * 4.0,
    )?;
    solver.add_balloon(triangles.to_vec(), pressure, volume_compliance);
    Some(solver)
}
