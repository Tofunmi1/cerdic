//! Grass blades from a tuft skeleton.
use super::{LodError, MaterialRange, PlantMesh, Surface, Vertex, WindData};
use crate::flora::Skeleton;

/// Meshes each tuft branch as a tapered blade with wind increasing from root to tip.
///
/// # Errors
/// Returns `BadParameter` when `blade_segments` is outside 2..=7.
pub fn mesh_tuft(skeleton: &Skeleton, blade_segments: u32) -> Result<PlantMesh, LodError> {
    if !(2..=7).contains(&blade_segments) {
        return Err(LodError::BadParameter {
            name: "blade_segments",
            why: "must be in 2..=7".to_owned(),
        });
    }
    let mut mesh = PlantMesh::default();
    for line in skeleton.polylines() {
        if line.len() < 2 {
            continue;
        }
        let first = line[0] as usize;
        let last = *line.last().unwrap_or(&line[0]) as usize;
        let start = skeleton.position[first];
        let end = skeleton.position[last];
        let axis = unit(sub(end, start));
        let across = perpendicular(axis);
        let normal = unit(cross(across, axis));
        let base_index = mesh.vertices.len();
        for segment in 0..=blade_segments {
            let t = f32::from(u16::try_from(segment).unwrap_or(0))
                / f32::from(u16::try_from(blade_segments).unwrap_or(1));
            let centre = add(start, scale(sub(end, start), t));
            let width = skeleton.radius.get(first).copied().unwrap_or(0.01) * (1.0 - t).max(0.08);
            for side in [-1.0, 1.0] {
                let position = add(centre, scale(across, width * side));
                mesh.vertices.push(Vertex {
                    position,
                    normal,
                    uv: [if side < 0.0 { 0.0 } else { 1.0 }, t],
                    tangent: [across[0], across[1], across[2], 1.0],
                    wind: WindData {
                        pivot: start,
                        phase: skeleton
                            .phase
                            .get(first)
                            .copied()
                            .unwrap_or(0.0)
                            .clamp(0.0, 1.0),
                        trunk: t,
                        branch: 0.0,
                        leaf: t,
                        edge: if segment == blade_segments { 1.0 } else { 0.0 },
                    },
                });
            }
            if segment > 0 {
                let old = base_index + ((segment - 1) * 2) as usize;
                let new = base_index + (segment * 2) as usize;
                let old_index = u32::try_from(old).unwrap_or(u32::MAX);
                let new_index = u32::try_from(new).unwrap_or(u32::MAX);
                mesh.indices.extend_from_slice(&[
                    old_index,
                    old_index + 1,
                    new_index + 1,
                    old_index,
                    new_index + 1,
                    new_index,
                ]);
            }
        }
    }
    if !mesh.indices.is_empty() {
        mesh.material_ranges.push(MaterialRange {
            kind: Surface::Leaf,
            first_index: 0,
            index_count: u32::try_from(mesh.indices.len()).unwrap_or(u32::MAX),
        });
    }
    Ok(mesh)
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(a: [f32; 3], amount: f32) -> [f32; 3] {
    [a[0] * amount, a[1] * amount, a[2] * amount]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn length(a: [f32; 3]) -> f32 {
    dot(a, a).sqrt()
}

fn unit(a: [f32; 3]) -> [f32; 3] {
    let length = length(a);
    if length > 1.0e-8 {
        scale(a, 1.0 / length)
    } else {
        [0.0, 1.0, 0.0]
    }
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn perpendicular(a: [f32; 3]) -> [f32; 3] {
    unit(if a[1].abs() < 0.9 {
        [a[2], 0.0, -a[0]]
    } else {
        [1.0, 0.0, 0.0]
    })
}
