//! Bevel disjoint manifold edges of valence-3 corners with a flat or rounded profile.

mod profile;
mod rebuild;

use std::collections::{BTreeMap, BTreeSet};

use ve_kernel::ExecutionContext;

use crate::topology::check_cancelled;
use crate::{Element, GeometryError, PolyMesh, Topology, validate};
use profile::refusal;

/// The most profile segments one bevel may use.
pub const MAX_BEVEL_SEGMENTS: usize = 4;

/// Bevels the selected edges by `width` with `segments` profile segments (1 is a flat chamfer, more
/// approximate a round).
///
/// V1 contract: every edge has exactly two planar faces that are not nearly coplanar; the edges share
/// no vertex; each end vertex has exactly three faces; `width` is positive and cuts less than 90 % of
/// any edge it runs along. Connected edge networks, boundary and non-manifold edges and higher-valence
/// corners are refused with a typed code naming the element.
///
/// # Errors
/// A typed refusal, a structural mesh error or cancellation. The input is never changed.
pub fn bevel_edges(
    mesh: &PolyMesh,
    edges: &[(u32, u32)],
    width: f64,
    segments: usize,
    context: &ExecutionContext,
) -> Result<PolyMesh, GeometryError> {
    let before = validate(mesh, context)?;
    let first = edges.first().copied();
    let element = first.map(|(a, b)| Element::Edge(a, b));
    if edges.is_empty() {
        return Err(GeometryError::PolyRefusal {
            code: "poly.selection_empty",
            element: None,
        });
    }
    if !width.is_finite() || width <= 0.0 {
        return Err(GeometryError::PolyRefusal {
            code: "poly.bevel_width_invalid",
            element,
        });
    }
    if segments == 0 || segments > MAX_BEVEL_SEGMENTS {
        return Err(GeometryError::PolyRefusal {
            code: "poly.bevel_segments_invalid",
            element,
        });
    }
    let topology = Topology::build(mesh, context)?;
    check_edges(&topology, edges, &before)?;
    let ranges = ranges(mesh)?;
    let mut consumed = BTreeMap::new();
    let mut plans = Vec::with_capacity(edges.len());
    for &edge in edges {
        check_cancelled(context)?;
        plans.push(profile::plan_edge(
            mesh,
            &topology,
            &ranges,
            edge,
            (width, segments),
            &mut consumed,
        )?);
    }
    let candidate = rebuild::build(mesh, &ranges, &plans, context)?;
    let after = validate(&candidate, context)?;
    if after.euler_characteristic != before.euler_characteristic
        || after.components != before.components
        || after.nonmanifold_edges.len() > before.nonmanifold_edges.len()
        || after.inconsistent_winding_edges.len() > before.inconsistent_winding_edges.len()
        || after.degenerate_faces.len() > before.degenerate_faces.len()
        || after.isolated_vertices > before.isolated_vertices
    {
        return Err(GeometryError::PolyRefusal {
            code: "poly.topology_unexpected",
            element,
        });
    }
    let new_faces: Vec<usize> = (0..candidate.face_count()).collect();
    let touched: Vec<usize> = new_faces
        .iter()
        .copied()
        .filter(|&f| f >= mesh.face_count() || plans.iter().any(|p| touches(p, f)))
        .collect();
    super::overlap::refuse_crossings(&candidate, &touched, context)?;
    Ok(candidate)
}

fn touches(plan: &profile::EdgePlan, face: usize) -> bool {
    plan.ends.iter().any(|end| end.faces.contains(&face))
}

fn ranges(mesh: &PolyMesh) -> Result<Vec<std::ops::Range<usize>>, GeometryError> {
    let mut start = 0_usize;
    mesh.face_sizes()
        .iter()
        .map(|&size| {
            let end = start
                .checked_add(size as usize)
                .ok_or(GeometryError::InvalidMesh("face range overflow"))?;
            let range = start..end;
            start = end;
            Ok(range)
        })
        .collect()
}

/// Edges exist, are manifold interior edges, touch no degenerate face and share no vertex.
fn check_edges(
    topology: &Topology,
    edges: &[(u32, u32)],
    report: &crate::ValidationReport,
) -> Result<(), GeometryError> {
    let mut seen = BTreeSet::new();
    for &(a, b) in edges {
        let key = (a.min(b), a.max(b));
        let element = Element::Edge(key.0, key.1);
        let Some(info) = topology.edges().get(&key) else {
            return Err(refusal("poly.selection_invalid", element));
        };
        match info.face_count() {
            2 => {}
            1 => return Err(refusal("poly.boundary_edge", element)),
            _ => return Err(refusal("poly.nonmanifold", element)),
        }
        if info.uses()[0].forward == info.uses()[1].forward {
            return Err(refusal("poly.winding_conflict", element));
        }
        if let Some(&face) = info
            .uses()
            .iter()
            .map(|u| &u.face)
            .find(|f| report.degenerate_faces.contains(f))
        {
            return Err(refusal("poly.face_degenerate", Element::Face(face)));
        }
        for vertex in [key.0, key.1] {
            if !seen.insert(vertex) {
                return Err(refusal(
                    "poly.bevel_network_unsupported",
                    Element::Vertex(vertex),
                ));
            }
        }
    }
    Ok(())
}
