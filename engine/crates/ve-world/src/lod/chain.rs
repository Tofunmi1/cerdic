//! Simpler meshes for distance.
use super::{LodChain, LodError, MaterialRange, PlantMesh, Surface, Vertex};
use crate::flora::Skeleton;

/// Builds meshes of falling detail from a full plant mesh.
///
/// # Errors
/// Returns `Cancelled` or `BadParameter` for an invalid level count.
pub fn build_lods(
    full: &PlantMesh,
    _skeleton: &Skeleton,
    levels: u32,
    cancel: &dyn Fn() -> bool,
) -> Result<LodChain, LodError> {
    if !(2..=5).contains(&levels) {
        return Err(LodError::BadParameter {
            name: "levels",
            why: "must be in 2..=5".to_owned(),
        });
    }
    if cancel() {
        return Err(LodError::Cancelled);
    }
    let mut chain = LodChain {
        levels: vec![full.clone()],
        switch_distance_per_size: Vec::new(),
    };
    for level in 1..levels {
        if cancel() {
            return Err(LodError::Cancelled);
        }
        let previous = chain.levels.last().cloned().unwrap_or_default();
        let simplified = simplify(&previous);
        chain.levels.push(simplified);
        let switch_distance = match level {
            1 => 8.0,
            2 => 16.0,
            3 => 24.0,
            _ => 32.0,
        };
        chain.switch_distance_per_size.push(switch_distance);
    }
    Ok(chain)
}

fn simplify(source: &PlantMesh) -> PlantMesh {
    let triangles = source.indices.len() / 3;
    let target = triangles / 2;
    let bark_count = count_triangles(source, Surface::Bark);
    let leaf_count = count_triangles(source, Surface::Leaf);
    let bark_target = (bark_count / 2).min(target);
    let leaf_target = target.saturating_sub(bark_target).min(leaf_count);
    let mut mesh = PlantMesh {
        vertices: source.vertices.clone(),
        indices: Vec::new(),
        material_ranges: Vec::new(),
    };
    append_surface(source, &mut mesh, Surface::Bark, bark_target, false);
    append_surface(source, &mut mesh, Surface::Leaf, leaf_target, true);
    mesh
}

fn append_surface(
    source: &PlantMesh,
    output: &mut PlantMesh,
    kind: Surface,
    target: usize,
    preserve_area: bool,
) {
    if target == 0 {
        return;
    }
    let source_triangles: Vec<[u32; 3]> = source
        .material_ranges
        .iter()
        .filter(|range| range.kind == kind)
        .flat_map(|range| {
            let start = range.first_index as usize;
            source.indices[start..start + range.index_count as usize]
                .chunks_exact(3)
                .map(|triangle| [triangle[0], triangle[1], triangle[2]])
        })
        .collect();
    let start = output.indices.len();
    for group in partition(source_triangles.len(), target) {
        if preserve_area {
            append_scaled_group(source, output, &source_triangles, &group);
        } else {
            output
                .indices
                .extend_from_slice(&source_triangles[group[0]]);
        }
    }
    output.material_ranges.push(MaterialRange {
        kind,
        first_index: u32::try_from(start).unwrap_or(u32::MAX),
        index_count: u32::try_from(output.indices.len() - start).unwrap_or(u32::MAX),
    });
}

fn partition(length: usize, groups: usize) -> Vec<Vec<usize>> {
    let mut result = Vec::with_capacity(groups);
    for group in 0..groups {
        let from = group * length / groups;
        let to = (group + 1) * length / groups;
        result.push((from..to).collect());
    }
    result
}

fn append_scaled_group(
    source: &PlantMesh,
    output: &mut PlantMesh,
    triangles: &[[u32; 3]],
    group: &[usize],
) {
    let mut area = 0.0;
    let mut first_triangle = [0_u32; 3];
    for (offset, &triangle_index) in group.iter().enumerate() {
        let indices = triangles[triangle_index];
        if offset == 0 {
            first_triangle = indices;
        }
        area += triangle_area(source, indices);
    }
    let original = first_triangle.map(|index| source.vertices[index as usize]);
    let centroid = average_position(&original);
    let triangle_area = triangle_area(source, first_triangle).max(1.0e-12);
    let scale = (area / triangle_area).sqrt();
    for vertex in original {
        let mut adjusted = vertex;
        for (axis, centre) in centroid.iter().enumerate() {
            adjusted.position[axis] = centre + (vertex.position[axis] - centre) * scale;
        }
        output.vertices.push(adjusted);
    }
    output.indices.extend_from_slice(&[
        u32::try_from(output.vertices.len() - 3).unwrap_or(u32::MAX),
        u32::try_from(output.vertices.len() - 2).unwrap_or(u32::MAX),
        u32::try_from(output.vertices.len() - 1).unwrap_or(u32::MAX),
    ]);
}

fn triangle_area(mesh: &PlantMesh, triangle: [u32; 3]) -> f32 {
    let first = mesh.vertices[triangle[0] as usize].position;
    let second = mesh.vertices[triangle[1] as usize].position;
    let third = mesh.vertices[triangle[2] as usize].position;
    let edge_one = [
        second[0] - first[0],
        second[1] - first[1],
        second[2] - first[2],
    ];
    let edge_two = [
        third[0] - first[0],
        third[1] - first[1],
        third[2] - first[2],
    ];
    let cross = [
        edge_one[1] * edge_two[2] - edge_one[2] * edge_two[1],
        edge_one[2] * edge_two[0] - edge_one[0] * edge_two[2],
        edge_one[0] * edge_two[1] - edge_one[1] * edge_two[0],
    ];
    0.5 * (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt()
}

fn average_position(vertices: &[Vertex; 3]) -> [f32; 3] {
    [
        (vertices[0].position[0] + vertices[1].position[0] + vertices[2].position[0]) / 3.0,
        (vertices[0].position[1] + vertices[1].position[1] + vertices[2].position[1]) / 3.0,
        (vertices[0].position[2] + vertices[1].position[2] + vertices[2].position[2]) / 3.0,
    ]
}

fn count_triangles(mesh: &PlantMesh, kind: Surface) -> usize {
    mesh.material_ranges
        .iter()
        .filter(|range| range.kind == kind)
        .map(|range| range.index_count as usize / 3)
        .sum()
}
