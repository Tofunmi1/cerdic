//! Builds the beveled mesh from the planned edge profiles.

use std::collections::BTreeSet;

use ve_kernel::ExecutionContext;

use super::profile::{EdgePlan, Vec3, loop_of};
use crate::poly::attrs::{Sources, transfer};
use crate::topology::check_cancelled;
use crate::{GeometryError, PolyMesh};

pub(super) fn build(
    mesh: &PolyMesh,
    ranges: &[std::ops::Range<usize>],
    plans: &[EdgePlan],
    context: &ExecutionContext,
) -> Result<PolyMesh, GeometryError> {
    let original = mesh.positions().len();
    let mut loops: Vec<Vec<u32>> = ranges
        .iter()
        .map(|r| mesh.corners()[r.clone()].to_vec())
        .collect();
    let mut corner_sources: Vec<Vec<usize>> = ranges.iter().map(|r| r.clone().collect()).collect();
    let mut new_points: Vec<(Vec3, u32)> = Vec::new();
    let mut ids: Vec<[Vec<u32>; 2]> = Vec::new();
    for plan in plans {
        check_cancelled(context)?;
        let mut pair = [Vec::new(), Vec::new()];
        for (slot, end) in plan.ends.iter().enumerate() {
            for p in &end.profile {
                pair[slot].push(index_after(original, new_points.len())?);
                new_points.push((*p, end.vertex));
            }
            substitute(&mut loops, &mut corner_sources, end, &pair[slot])?;
        }
        ids.push(pair);
    }
    let mut face_sources: Vec<usize> = (0..loops.len()).collect();
    for (plan, pair) in plans.iter().zip(&ids) {
        strips(
            mesh,
            ranges,
            plan,
            pair,
            (&mut loops, &mut corner_sources, &mut face_sources),
        )?;
    }
    let removed: BTreeSet<u32> = plans
        .iter()
        .flat_map(|plan| plan.ends.iter().map(|end| end.vertex))
        .collect();
    compact(
        mesh,
        loops,
        corner_sources,
        face_sources,
        (&new_points, &removed),
        context,
    )
}

fn index_after(original: usize, added: usize) -> Result<u32, GeometryError> {
    u32::try_from(original + added)
        .map_err(|_| GeometryError::InvalidMesh("vertex index exceeds u32"))
}

/// Replaces an end's vertex in its three faces: the first and second faces take one profile point
/// each, the third takes the whole profile in the order that follows its winding.
fn substitute(
    loops: &mut [Vec<u32>],
    corner_sources: &mut [Vec<usize>],
    end: &super::profile::End,
    new_ids: &[u32],
) -> Result<(), GeometryError> {
    let [first, second, third] = end.faces;
    let last = new_ids.len() - 1;
    for (face, id) in [(first, new_ids[0]), (second, new_ids[last])] {
        let at = loops[face]
            .iter()
            .position(|&v| v == end.vertex)
            .ok_or(GeometryError::InvalidMesh("bevel vertex missing from face"))?;
        loops[face][at] = id;
    }
    let at = loops[third]
        .iter()
        .position(|&v| v == end.vertex)
        .ok_or(GeometryError::InvalidMesh("bevel vertex missing from face"))?;
    let chain: Vec<u32> = if end.chain_in_order {
        new_ids.to_vec()
    } else {
        new_ids.iter().rev().copied().collect()
    };
    let source = corner_sources[third][at];
    loops[third].splice(at..=at, chain.iter().copied());
    corner_sources[third].splice(at..=at, std::iter::repeat_n(source, chain.len()));
    Ok(())
}

/// Adds one quad per profile segment between the two ends, wound against the first face's edge.
fn strips(
    mesh: &PolyMesh,
    ranges: &[std::ops::Range<usize>],
    plan: &EdgePlan,
    pair: &[Vec<u32>; 2],
    (loops, corner_sources, face_sources): (
        &mut Vec<Vec<u32>>,
        &mut Vec<Vec<usize>>,
        &mut Vec<usize>,
    ),
) -> Result<(), GeometryError> {
    let [end_u, end_v] = &plan.ends;
    let corner_at = |face: usize, vertex: u32| -> Result<usize, GeometryError> {
        let range = &ranges[face];
        let original = loop_of(mesh, ranges, face)
            .iter()
            .position(|&v| v == vertex)
            .ok_or(GeometryError::InvalidMesh("bevel vertex missing from face"))?;
        Ok(range.start + original)
    };
    let sources = [
        corner_at(end_u.faces[0], end_v.vertex)?,
        corner_at(end_u.faces[0], end_u.vertex)?,
        corner_at(end_u.faces[1], end_u.vertex)?,
        corner_at(end_u.faces[1], end_v.vertex)?,
    ];
    for i in 0..pair[0].len() - 1 {
        let quad = [pair[1][i], pair[0][i], pair[0][i + 1], pair[1][i + 1]];
        let (quad, sources) = if plan.first_forward {
            (quad, sources)
        } else {
            (
                [quad[3], quad[2], quad[1], quad[0]],
                [sources[3], sources[2], sources[1], sources[0]],
            )
        };
        loops.push(quad.to_vec());
        corner_sources.push(sources.to_vec());
        face_sources.push(end_u.faces[0]);
    }
    Ok(())
}

/// Drops the replaced vertices, renumbers, and assembles the candidate with its attributes.
fn compact(
    mesh: &PolyMesh,
    loops: Vec<Vec<u32>>,
    corner_sources: Vec<Vec<usize>>,
    face_sources: Vec<usize>,
    (new_points, removed): (&[(Vec3, u32)], &BTreeSet<u32>),
    context: &ExecutionContext,
) -> Result<PolyMesh, GeometryError> {
    let original = mesh.positions().len();
    let mut renumber = vec![u32::MAX; original + new_points.len()];
    let mut positions = Vec::new();
    let mut point_sources = Vec::new();
    for (vertex, slot) in renumber.iter_mut().enumerate().take(original) {
        let id = u32::try_from(vertex).map_err(|_| GeometryError::InvalidMesh("vertex id"))?;
        if !removed.contains(&id) {
            *slot = u32::try_from(positions.len())
                .map_err(|_| GeometryError::InvalidMesh("vertex index exceeds u32"))?;
            positions.push(mesh.positions()[vertex]);
            point_sources.push(vertex);
        }
    }
    for (offset, (position, source)) in new_points.iter().enumerate() {
        renumber[original + offset] = u32::try_from(positions.len())
            .map_err(|_| GeometryError::InvalidMesh("vertex index exceeds u32"))?;
        positions.push(position.map(to_f32));
        point_sources.push(*source as usize);
    }
    let sizes: Vec<u32> = loops
        .iter()
        .map(|l| u32::try_from(l.len()).unwrap_or(u32::MAX))
        .collect();
    let corners: Vec<u32> = loops
        .iter()
        .flatten()
        .map(|&v| renumber[v as usize])
        .collect();
    let mut candidate = PolyMesh::new(positions, sizes, corners)?;
    transfer(
        mesh,
        &mut candidate,
        &Sources {
            points: point_sources,
            faces: face_sources,
            corners: corner_sources.into_iter().flatten().collect(),
        },
        context,
    )?;
    Ok(candidate)
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "polygon positions are f32 by contract"
)]
fn to_f32(value: f64) -> f32 {
    value as f32
}
