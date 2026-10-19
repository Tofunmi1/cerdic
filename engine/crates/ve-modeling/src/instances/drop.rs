//! `inst.drop`: lowers each placement straight down onto a surface solid (trees on a hill, rocks on a plain).
//!
//! The surface's triangles are bucketed on an XZ grid so the cost is near linear in the number of placements; the hit is
//! the highest surface point under the placement. Only `+ - * /` and the tilt rotation are used.
use super::InstanceSet;
use crate::{Fail, fail, profile_ops::number};
use glam::{DMat4, DQuat, DVec3};
use std::collections::HashMap;
use ve_graph::{Node, Value as Param};
use ve_kernel::Mesh;

/// How far outside a triangle's edge a ray still counts as inside, so a ray on a shared edge hits one of the two.
const EDGE: f64 = -1.0e-9;

struct Triangle {
    corners: [DVec3; 3],
}

impl Triangle {
    /// Height of the surface at `(x, z)` if the point is under the triangle's footprint.
    fn height_at(&self, east: f64, north: f64) -> Option<f64> {
        let [first, second, third] = self.corners;
        let denominator =
            (second.z - third.z) * (first.x - third.x) + (third.x - second.x) * (first.z - third.z);
        if denominator.abs() < 1.0e-12 {
            return None;
        }
        let weight_first = ((second.z - third.z) * (east - third.x)
            + (third.x - second.x) * (north - third.z))
            / denominator;
        let weight_second = ((third.z - first.z) * (east - third.x)
            + (first.x - third.x) * (north - third.z))
            / denominator;
        let weight_third = 1.0 - weight_first - weight_second;
        (weight_first >= EDGE && weight_second >= EDGE && weight_third >= EDGE)
            .then_some(weight_first * first.y + weight_second * second.y + weight_third * third.y)
    }

    fn normal(&self) -> DVec3 {
        let [first, second, third] = self.corners;
        (second - first).cross(third - first).normalize_or_zero()
    }
}

/// The triangles of a mesh on an XZ grid.
struct Ground {
    triangles: Vec<Triangle>,
    cells: HashMap<(i64, i64), Vec<usize>>,
    cell: f64,
    top: f64,
}

impl Ground {
    fn new(mesh: &Mesh) -> Self {
        let triangles: Vec<Triangle> = mesh
            .indices
            .chunks_exact(3)
            .map(|t| Triangle {
                corners: [0, 1, 2]
                    .map(|k| DVec3::from_array(mesh.positions[t[k] as usize].map(f64::from))),
            })
            .collect();
        let (mut lo, mut hi) = (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY));
        for p in &mesh.positions {
            let p = DVec3::from_array(p.map(f64::from));
            lo = lo.min(p);
            hi = hi.max(p);
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "triangle counts are far below 2^52"
        )]
        let cell = ((hi.x - lo.x) * (hi.z - lo.z) / (triangles.len().max(1) as f64))
            .sqrt()
            .max(1.0e-3)
            * 2.0;
        let mut cells: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (index, triangle) in triangles.iter().enumerate() {
            let min = triangle
                .corners
                .iter()
                .fold(DVec3::splat(f64::INFINITY), |m, p| m.min(*p));
            let max = triangle
                .corners
                .iter()
                .fold(DVec3::splat(f64::NEG_INFINITY), |m, p| m.max(*p));
            for i in bucket(min.x, cell)..=bucket(max.x, cell) {
                for j in bucket(min.z, cell)..=bucket(max.z, cell) {
                    cells.entry((i, j)).or_default().push(index);
                }
            }
        }
        Self {
            triangles,
            cells,
            cell,
            top: hi.y,
        }
    }

    /// The highest surface point under `(x, z)` and the normal there.
    fn hit(&self, x: f64, z: f64) -> Option<(f64, DVec3)> {
        let candidates = self
            .cells
            .get(&(bucket(x, self.cell), bucket(z, self.cell)))?;
        candidates
            .iter()
            .filter_map(|index| self.triangles[*index].height_at(x, z).map(|y| (y, *index)))
            // Highest first; equal heights keep the lower triangle index, so the result never depends on a tie.
            .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)))
            .map(|(y, index)| (y, self.triangles[index].normal()))
    }
}

fn bucket(value: f64, cell: f64) -> i64 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "coordinates are bounded by the kernel extent"
    )]
    let index = (value / cell).floor() as i64;
    index
}

/// Places every instance of `base` on `surface`; `None` placements that miss it are dropped or kept as authored.
pub(super) fn drop_onto(
    node: &Node,
    base: &InstanceSet,
    surface: &Mesh,
) -> Result<Vec<DMat4>, Fail> {
    if surface.is_empty() {
        return Err(fail(
            "graph.missing_input",
            "inst.drop needs a surface that is not empty",
        ));
    }
    let offset =
        number(node, "offset").map_err(|_| fail("param.type", "inst.drop offset is missing"))?;
    let align = number(node, "align")
        .map_err(|_| fail("param.type", "inst.drop align is missing"))?
        .clamp(0.0, 1.0);
    let keep_misses =
        matches!(node.params.get("outside"), Some(Param::Str(name)) if name == "keep");
    let ground = Ground::new(surface);
    let mut placed = Vec::with_capacity(base.transforms.len());
    for matrix in &base.transforms {
        let at = matrix.w_axis.truncate();
        let Some((height, normal)) = ground.hit(at.x, at.z) else {
            if keep_misses {
                placed.push(*matrix);
            }
            continue;
        };
        let mut linear = *matrix;
        linear.w_axis = glam::DVec4::new(0.0, 0.0, 0.0, 1.0);
        let turn = if align > 0.0 && normal.y > 1.0e-6 {
            let angle = normal.y.clamp(-1.0, 1.0).acos() * align;
            let axis = DVec3::Y.cross(normal);
            if axis.length_squared() > 1.0e-18 {
                DMat4::from_quat(DQuat::from_axis_angle(axis.normalize(), angle))
            } else {
                DMat4::IDENTITY
            }
        } else {
            DMat4::IDENTITY
        };
        let up = if align > 0.0 {
            turn.transform_vector3(DVec3::Y)
        } else {
            DVec3::Y
        };
        placed.push(
            DMat4::from_translation(DVec3::new(at.x, height, at.z) + up * offset) * turn * linear,
        );
    }
    if placed.is_empty() {
        return Err(fail(
            "geometry.no_placements",
            format!(
                "none of the {} placements is over the surface (it spans up to y={:.2}); scatter inside the surface's footprint or set outside=\"keep\"",
                base.transforms.len(),
                ground.top
            ),
        ));
    }
    Ok(placed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slope() -> Mesh {
        // A single sloped quad from y=0 at x=-1 to y=2 at x=1, over z in -1..1, two triangles facing up.
        Mesh {
            positions: vec![
                [-1.0, 0.0, -1.0],
                [-1.0, 0.0, 1.0],
                [1.0, 2.0, -1.0],
                [1.0, 2.0, 1.0],
            ],
            indices: vec![0, 1, 2, 1, 3, 2],
        }
    }

    #[test]
    fn height_follows_the_slope_and_misses_outside() {
        let ground = Ground::new(&slope());
        let (y, normal) = ground.hit(0.0, 0.3).unwrap_or((f64::NAN, DVec3::ZERO));
        assert!((y - 1.0).abs() < 1e-9, "{y}");
        assert!(
            normal.y > 0.0 && normal.x < 0.0,
            "the slope faces up and toward -x: {normal:?}"
        );
        assert!(ground.hit(1.5, 0.0).is_none() && ground.hit(0.0, -2.0).is_none());
        let (edge, _) = ground.hit(0.0, 0.0).unwrap_or((f64::NAN, DVec3::ZERO));
        assert!(
            (edge - 1.0).abs() < 1e-9,
            "a ray exactly on the shared diagonal still hits"
        );
    }
}
