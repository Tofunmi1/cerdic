//! Find named meshes that are separated from every other part.

use std::fmt::Write as _;

use glam::Vec3;

use crate::bvh::Bvh;
use crate::error::MeasureError;
use crate::mesh::Mesh;
use crate::report::Report;

/// Maximum number of parts accepted by [`attachments`].
pub const MAX_PARTS: usize = 64;
/// Maximum combined triangle count accepted by [`attachments`].
pub const MAX_ATTACHMENT_TRIANGLES: usize = 2_000_000;

/// One part of a scene: a name and its mesh (world positions, triangle indices).
#[derive(Clone, Copy, Debug)]
pub struct NamedMesh<'a> {
    /// Part name.
    pub name: &'a str,
    /// Part mesh in world positions.
    pub mesh: Mesh<'a>,
}

/// How one part relates to the others.
#[derive(Clone, Debug, PartialEq)]
pub struct Attachment {
    /// This part's name.
    pub name: String,
    /// Smallest surface distance to another part, or zero for contact/overlap.
    pub gap: f32,
    /// Nearest other part, empty when there is only one part.
    pub nearest: String,
}

/// Measure each part's true minimum surface gap to another part.
///
/// Results stay in input order. A part is attached when its gap to at least one other part is at
/// most `tolerance`; callers can identify floating parts by comparing the returned gaps.
///
/// # Errors
/// Returns a typed error for malformed or empty meshes, invalid tolerance, or exceeded budgets.
pub fn attachments(
    parts: &[NamedMesh<'_>],
    tolerance: f32,
) -> Result<Vec<Attachment>, MeasureError> {
    if parts.len() > MAX_PARTS {
        return Err(MeasureError::PartBudget {
            parts: parts.len(),
            limit: MAX_PARTS,
        });
    }
    if !tolerance.is_finite() || tolerance < 0.0 {
        return Err(MeasureError::Degenerate(
            "tolerance must be finite and non-negative",
        ));
    }
    let mut triangle_total = 0usize;
    let mut trees = Vec::with_capacity(parts.len());
    let mut closed = Vec::with_capacity(parts.len());
    for part in parts {
        let count = part.mesh.validate()?;
        triangle_total = triangle_total.saturating_add(count);
        if triangle_total > MAX_ATTACHMENT_TRIANGLES {
            return Err(MeasureError::TriangleBudget {
                triangles: triangle_total,
                limit: MAX_ATTACHMENT_TRIANGLES,
            });
        }
        trees.push(Bvh::build(&part.mesh)?);
        closed.push(Report::of(&part.mesh)?.watertight);
    }
    let mut out = Vec::with_capacity(parts.len());
    for i in 0..parts.len() {
        let mut best = f32::INFINITY;
        let mut nearest = "";
        for j in 0..parts.len() {
            if i == j {
                continue;
            }
            let gap = pair_gap(
                parts[i].mesh,
                &trees[i],
                closed[i],
                parts[j].mesh,
                &trees[j],
                closed[j],
            );
            if gap < best {
                best = gap;
                nearest = parts[j].name;
            }
        }
        out.push(Attachment {
            name: parts[i].name.to_string(),
            gap: if best.is_finite() { best } else { 0.0 },
            nearest: nearest.to_string(),
        });
    }
    Ok(out)
}

/// Work allowance of [`attachments_json`]: closest-point queries and ray casts together. A browser does about a million a second.
pub const ATTACHMENT_WORK_BUDGET: usize = 600_000;

type Bounds = ([f32; 3], [f32; 3]);

fn bounds(positions: &[[f32; 3]]) -> Bounds {
    positions.iter().fold(([f32::MAX; 3], [f32::MIN; 3]), |(lo, hi), p| {
        (
            [lo[0].min(p[0]), lo[1].min(p[1]), lo[2].min(p[2])],
            [hi[0].max(p[0]), hi[1].max(p[1]), hi[2].max(p[2])],
        )
    })
}

/// Distance between two boxes, zero when they overlap: no surface of one can be closer to the other than this.
fn box_gap(a: &Bounds, b: &Bounds) -> f32 {
    (0..3)
        .map(|k| (a.0[k] - b.1[k]).max(b.0[k] - a.1[k]).max(0.0).powi(2))
        .sum::<f32>()
        .sqrt()
}

fn spend(spent: &mut usize, steps: usize, budget: usize) -> Result<(), MeasureError> {
    *spent = spent.saturating_add(steps);
    if *spent > budget {
        return Err(MeasureError::WorkBudget { limit: budget });
    }
    Ok(())
}

/// Like [`attachments`], but built for big scenes: it stops looking for a part's neighbour as soon as one is within `tolerance`, tries the nearest
/// bounding boxes first and skips boxes that cannot beat the best gap, so an attached part costs a few queries rather than a scan of every other
/// part. A part within `tolerance` reports the first gap found at or under it (not the exact minimum); a floating part still reports its exact gap and
/// nearest part. `budget` caps closest-point queries and ray casts together.
///
/// # Errors
/// The errors of [`attachments`], and [`MeasureError::WorkBudget`] when the work allowance runs out.
pub fn attachments_within(
    parts: &[NamedMesh<'_>],
    tolerance: f32,
    budget: usize,
) -> Result<Vec<Attachment>, MeasureError> {
    if parts.len() > MAX_PARTS {
        return Err(MeasureError::PartBudget { parts: parts.len(), limit: MAX_PARTS });
    }
    if !tolerance.is_finite() || tolerance < 0.0 {
        return Err(MeasureError::Degenerate("tolerance must be finite and non-negative"));
    }
    let mut triangle_total = 0usize;
    let (mut trees, mut closed, mut boxes) = (Vec::new(), Vec::new(), Vec::new());
    for part in parts {
        triangle_total = triangle_total.saturating_add(part.mesh.validate()?);
        if triangle_total > MAX_ATTACHMENT_TRIANGLES {
            return Err(MeasureError::TriangleBudget { triangles: triangle_total, limit: MAX_ATTACHMENT_TRIANGLES });
        }
        trees.push(Bvh::build(&part.mesh)?);
        closed.push(Report::of(&part.mesh)?.watertight);
        boxes.push(bounds(part.mesh.positions));
    }
    let mut spent = 0usize;
    let mut out = Vec::with_capacity(parts.len());
    for i in 0..parts.len() {
        let mut others: Vec<(f32, usize)> = (0..parts.len()).filter(|j| *j != i).map(|j| (box_gap(&boxes[i], &boxes[j]), j)).collect();
        others.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (mut best, mut nearest) = (f32::INFINITY, "");
        for (lower_bound, j) in others {
            if best <= tolerance || lower_bound >= best {
                break;
            }
            let gap = pair_gap_within(
                (parts[i].mesh, &trees[i], closed[i]),
                (parts[j].mesh, &trees[j], closed[j]),
                lower_bound == 0.0,
                tolerance,
                (&mut spent, budget),
            )?;
            if gap < best {
                best = gap;
                nearest = parts[j].name;
            }
        }
        out.push(Attachment {
            name: parts[i].name.to_string(),
            gap: if best.is_finite() { best } else { 0.0 },
            nearest: nearest.to_string(),
        });
    }
    Ok(out)
}

type Side<'a, 'b> = (Mesh<'a>, &'b Bvh, bool);

/// [`pair_gap`] that returns as soon as the gap is within `tolerance`, skips the edge test when the boxes are apart, and pays for its work.
fn pair_gap_within(
    a: Side<'_, '_>,
    b: Side<'_, '_>,
    boxes_overlap: bool,
    tolerance: f32,
    (spent, budget): (&mut usize, usize),
) -> Result<f32, MeasureError> {
    let gap = a.1.proximity(&a.0, b.1, &b.0, tolerance, (&mut *spent, budget))?;
    if gap <= tolerance {
        return Ok(gap);
    }
    // One part inside a closed other has no surface near it: its first vertex is enough to tell.
    for (from, to) in [(a, b), (b, a)] {
        if let (true, Some(p)) = (to.2, from.0.positions.first()) {
            spend(spent, 8, budget)?;
            if inside(&to.0, to.1, *p) {
                return Ok(0.0);
            }
        }
    }
    if boxes_overlap {
        spend(spent, 3 * (a.0.indices.len() / 3 + b.0.indices.len() / 3), budget)?;
        if edges_intersect(a.0, b.0, b.1) || edges_intersect(b.0, a.0, a.1) {
            return Ok(0.0);
        }
    }
    Ok(gap)
}

/// JSON adapter returning only the floating parts, ordered largest-gap first.
#[must_use]
pub fn attachments_json(parts: &[NamedMesh<'_>], tolerance: f32) -> String {
    let measured = match attachments_within(parts, tolerance, ATTACHMENT_WORK_BUDGET) {
        Ok(measured) => measured,
        Err(error) => {
            return format!(
                "{{\"error\":\"{}\",\"suggestion\":\"check the meshes and attachment budgets\"}}",
                escape(&error.to_string())
            );
        }
    };
    let mut floating: Vec<_> = measured
        .into_iter()
        .filter(|item| item.gap > tolerance)
        .collect();
    floating.sort_by(|a, b| b.gap.total_cmp(&a.gap).then_with(|| a.name.cmp(&b.name)));
    let entries = floating
        .iter()
        .map(|item| {
            format!(
                "{{\"name\":\"{}\",\"gap\":{},\"nearest\":\"{}\"}}",
                escape(&item.name),
                item.gap,
                escape(&item.nearest)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let suggestion = floating.first().map_or_else(
        || "all parts are attached within tolerance".to_string(),
        |item| {
            format!(
                "move the root of `{}` {:.2} m toward `{}`, or sink it inside the body",
                item.name, item.gap, item.nearest
            )
        },
    );
    format!(
        "{{\"floating\":[{entries}],\"suggestion\":\"{}\"}}",
        escape(&suggestion)
    )
}

fn pair_gap(
    a: Mesh<'_>,
    a_tree: &Bvh,
    a_closed: bool,
    b: Mesh<'_>,
    b_tree: &Bvh,
    b_closed: bool,
) -> f32 {
    let mut gap = f32::INFINITY;
    for &p in a.positions {
        gap = gap.min(
            b_tree
                .closest_point(&b, p)
                .map_or(f32::INFINITY, |cp| cp.distance),
        );
        if b_closed && inside(&b, b_tree, p) {
            return 0.0;
        }
        if gap == 0.0 {
            return 0.0;
        }
    }
    for &p in b.positions {
        gap = gap.min(
            a_tree
                .closest_point(&a, p)
                .map_or(f32::INFINITY, |cp| cp.distance),
        );
        if a_closed && inside(&a, a_tree, p) {
            return 0.0;
        }
        if gap == 0.0 {
            return 0.0;
        }
    }
    if edges_intersect(a, b, b_tree) || edges_intersect(b, a, a_tree) {
        0.0
    } else {
        gap
    }
}

fn inside(mesh: &Mesh<'_>, tree: &Bvh, point: [f32; 3]) -> bool {
    let mut origin = Vec3::from_array(point);
    let mut count = 0usize;
    for _ in 0..=mesh.indices.len() / 3 {
        let Some(hit) = tree.raycast(mesh, origin.to_array(), Vec3::X.to_array()) else {
            break;
        };
        count += 1;
        origin = Vec3::from_array(hit.point) + Vec3::X * 1e-5;
    }
    count % 2 == 1
}

fn edges_intersect(a: Mesh<'_>, b: Mesh<'_>, tree: &Bvh) -> bool {
    for tri in 0..a.indices.len() / 3 {
        let (p0, p1, p2) = a.corners(tri);
        for (start, end) in [(p0, p1), (p1, p2), (p2, p0)] {
            let edge = end - start;
            let length = edge.length();
            if length > 0.0
                && tree
                    .raycast(&b, start.to_array(), (edge / length).to_array())
                    .is_some_and(|hit| hit.distance <= length + 1e-6)
            {
                return true;
            }
        }
    }
    false
}

fn escape(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}
