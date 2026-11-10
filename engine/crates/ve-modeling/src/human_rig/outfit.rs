//! `body.outfit`: real clothes and hair from the imported CC0 library, fitted to the body.
//!
//! The asset stores, for every vertex, the body triangle nearest to it when it was made and its offset in that triangle's frame. Fitting evaluates
//! `sum(weights * body vertices) + offset in the frame of the current body triangle`, so the garment follows every phenotype slider (height, weight, muscle).
use std::rc::Rc;

use ve_graph::{Node, Value as Param};
use ve_kernel::Mesh;

use super::{bone_list, fail, fail_suggest, pack_or_fail, rig_of, skinning, text, weights_from};
use crate::{Fail, cache::Value};

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit(a: [f32; 3]) -> [f32; 3] {
    let length = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-12);
    [a[0] / length, a[1] / length, a[2] / length]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Barycentric weights of the point of triangle `abc` closest to `p` (Ericson, Real-Time Collision Detection).
fn closest_weights(p: [f32; 3], a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let (ab, ac, ap) = (sub(b, a), sub(c, a), sub(p, a));
    let (d1, d2) = (dot(ab, ap), dot(ac, ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return [1.0, 0.0, 0.0];
    }
    let bp = sub(p, b);
    let (d3, d4) = (dot(ab, bp), dot(ac, bp));
    if d3 >= 0.0 && d4 <= d3 {
        return [0.0, 1.0, 0.0];
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return [1.0 - v, v, 0.0];
    }
    let cp = sub(p, c);
    let (d5, d6) = (dot(ab, cp), dot(ac, cp));
    if d6 >= 0.0 && d5 <= d6 {
        return [0.0, 0.0, 1.0];
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return [1.0 - w, 0.0, w];
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return [0.0, 1.0 - w, w];
    }
    let denom = 1.0 / (va + vb + vc);
    let (v, w) = (vb * denom, vc * denom);
    [1.0 - v - w, v, w]
}

/// The body's surface as a grid of vertices with their triangles, to find the nearest skin to a point.
struct Skin<'a> {
    template: &'a [[f32; 3]],
    normals: &'a [[f32; 3]],
    triangles: Vec<[usize; 3]>,
    incident: Vec<Vec<u32>>,
    grid: std::collections::HashMap<(i32, i32, i32), Vec<u32>>,
}

const CELL: f32 = 0.05;

fn cell_of(p: [f32; 3]) -> (i32, i32, i32) {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a grid cell of a person-sized body"
    )]
    let at = |v: f32| (v / CELL).floor() as i32;
    (at(p[0]), at(p[1]), at(p[2]))
}

impl<'a> Skin<'a> {
    fn new(template: &'a [[f32; 3]], normals: &'a [[f32; 3]], triangles: Vec<[usize; 3]>) -> Self {
        let mut incident = vec![Vec::new(); template.len()];
        for (t, tri) in triangles.iter().enumerate() {
            for v in tri {
                incident[*v].push(u32::try_from(t).unwrap_or(0));
            }
        }
        let mut grid: std::collections::HashMap<(i32, i32, i32), Vec<u32>> =
            std::collections::HashMap::new();
        for (v, p) in template.iter().enumerate() {
            grid.entry(cell_of(*p))
                .or_default()
                .push(u32::try_from(v).unwrap_or(0));
        }
        Self {
            template,
            normals,
            triangles,
            incident,
            grid,
        }
    }

    /// The signed distance from the nearest skin (positive outside) and the outward normal there, for a point within a few centimetres of the body.
    fn nearest(&self, p: [f32; 3]) -> Option<(f32, [f32; 3])> {
        let (cx, cy, cz) = cell_of(p);
        let mut best: Option<(f32, usize, [f32; 3])> = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let Some(vertices) = self.grid.get(&(cx + dx, cy + dy, cz + dz)) else {
                        continue;
                    };
                    for v in vertices {
                        for t in &self.incident[*v as usize] {
                            let [a, b, c] = self.triangles[*t as usize].map(|i| self.template[i]);
                            let w = closest_weights(p, a, b, c);
                            let q: [f32; 3] =
                                std::array::from_fn(|k| w[0] * a[k] + w[1] * b[k] + w[2] * c[k]);
                            let d = sub(p, q);
                            let distance = dot(d, d);
                            if best.is_none_or(|(old, _, _)| distance < old) {
                                best = Some((distance, *t as usize, w));
                            }
                        }
                    }
                }
            }
        }
        let (_, t, w) = best?;
        let [a, b, c] = self.triangles[t];
        let n = unit(std::array::from_fn(|k| {
            w[0] * self.normals[a][k] + w[1] * self.normals[b][k] + w[2] * self.normals[c][k]
        }));
        let q: [f32; 3] = std::array::from_fn(|k| {
            w[0] * self.template[a][k] + w[1] * self.template[b][k] + w[2] * self.template[c][k]
        });
        Some((dot(sub(p, q), n), n))
    }
}

pub(super) fn run(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let (body, rig) = rig_of(node, inputs)?;
    let human = pack_or_fail()?;
    let asset = text(node, "asset");
    let outfit = match human.outfit(asset) {
        Ok(Some(outfit)) => outfit,
        Ok(None) => {
            return Err(fail_suggest(
                "param.invalid",
                format!("{asset:?} is not a clothes or hair asset in the pack"),
                format!("the assets are: {}", human.outfit_names().join(", ")),
            ));
        }
        Err(e) => return Err(fail("human.outfit", e.to_string())),
    };
    let offset = match node.params.get("offset") {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "an offset of a few centimetres"
        )]
        Some(Param::Num(n)) => *n as f32,
        _ => 0.0,
    };
    let standoff = match node.params.get("standoff") {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a standoff of a few millimetres"
        )]
        Some(Param::Num(n)) => *n as f32,
        _ => 0.004,
    };
    let source = &body.human_source_indices;
    let positions = &body.mesh.positions;
    if source.len() != positions.len()
        || source
            .iter()
            .any(|vertex| *vertex as usize >= human.skin().len())
    {
        return Err(fail(
            "human.skin_mesh",
            "the body's surface changed after body.human, so an outfit cannot be fitted to it",
        ));
    }
    // The body's template vertices at their current (morphed) places, and the render vertex that carries each of them.
    let vertex_count = source.iter().copied().max().map_or(0, |m| m as usize + 1);
    let mut template = vec![[0.0_f32; 3]; vertex_count];
    let mut render_of = vec![usize::MAX; vertex_count];
    for (r, t) in source.iter().enumerate() {
        let t = *t as usize;
        if render_of[t] == usize::MAX {
            render_of[t] = r;
            template[t] = positions[r];
        }
    }
    // Outward vertex normals of the body, from its template triangles: the same definition the converter bound the asset with.
    let mut normals = vec![[0.0_f32; 3]; vertex_count];
    for tri in body.mesh.indices.chunks_exact(3) {
        let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| source[i as usize] as usize);
        let face = cross(sub(template[b], template[a]), sub(template[c], template[a]));
        for v in [a, b, c] {
            for k in 0..3 {
                normals[v][k] += face[k];
            }
        }
    }
    for n in &mut normals {
        *n = unit(*n);
    }
    let fitted: Vec<[f32; 3]> = outfit
        .ids
        .iter()
        .zip(&outfit.bary)
        .zip(&outfit.local)
        .map(|((ids, w), local)| {
            let [a, b, c] = ids.map(|i| template[i as usize]);
            let [na, nb, nc] = ids.map(|i| normals[i as usize]);
            let n = unit(std::array::from_fn(|k| {
                w[0] * na[k] + w[1] * nb[k] + w[2] * nc[k]
            }));
            let edge = sub(b, a);
            let along = edge[0] * n[0] + edge[1] * n[1] + edge[2] * n[2];
            let e1 = unit(std::array::from_fn(|k| edge[k] - along * n[k]));
            let e2 = cross(n, e1);
            let surface: [f32; 3] =
                std::array::from_fn(|k| w[0] * a[k] + w[1] * b[k] + w[2] * c[k]);
            // Cloth made for a slimmer body would sink into this one: it never goes closer to the skin than `standoff` below its own distance.
            let height = (local[2] + offset).max(standoff);
            std::array::from_fn(|k| {
                surface[k] + local[0] * e1[k] + local[1] * e2[k] + height * n[k]
            })
        })
        .collect();
    let skin_surface = Skin::new(
        &template,
        &normals,
        body.mesh
            .indices
            .chunks_exact(3)
            .map(|t| [t[0], t[1], t[2]].map(|i| source[i as usize] as usize))
            .collect(),
    );
    // Wherever the cloth ended up closer to the nearest skin than `standoff` (or inside it), push it out along the skin's normal.
    let fitted: Vec<[f32; 3]> = fitted
        .into_iter()
        .map(|p| match skin_surface.nearest(p) {
            Some((d, n)) if d < standoff && d > -0.16 => {
                std::array::from_fn(|k| p[k] + (standoff - d) * n[k])
            }
            _ => p,
        })
        .collect();
    let mut value = Value::solid(
        Mesh {
            positions: fitted,
            indices: outfit.indices.clone(),
        },
        0.0,
    );
    value.uvs = outfit.uvs.clone();
    let bones = bone_list(node, "body.outfit")?;
    if !bones.is_empty() {
        let (joints, weights) = weights_from(&rig, &bones, outfit.skin.iter().copied())?;
        value.skin = Some(skinning(node, bones, joints, weights));
    }
    Ok(value)
}
