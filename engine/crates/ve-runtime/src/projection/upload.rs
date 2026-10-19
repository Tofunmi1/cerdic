//! Turning one staged chunk of vertices into a GPU mesh asset.
use super::{Chunk, Uploaded};
use bevy_asset::{Assets, RenderAssetUsages};
use bevy_mesh::{Mesh, PrimitiveTopology};
use ve_kernel::Bounds;

/// Builds the mesh of `chunk` and its bounds. Vertices are seven floats (position, normal, occlusion), or nine
/// with texture coordinates for a part that has baked maps.
///
/// A skinned part passes `corners`, how many corners of it were uploaded before; the chunk keeps its rest data and
/// the offset, and the count moves on.
pub(super) fn upload(
    chunk: Chunk,
    meshes: &mut Assets<Mesh>,
    corners: Option<&mut usize>,
) -> Uploaded {
    let keep_rest = corners.is_some();
    let mut rest = Vec::new();
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut bounds = Bounds {
        min: [f64::INFINITY; 3],
        max: [f64::NEG_INFINITY; 3],
    };
    let mut occlusion = Vec::new();
    let mut uvs = Vec::new();
    let stride = if chunk.textured { 9 } else { 7 };
    for vertex in chunk.data.chunks_exact(stride) {
        positions.push([vertex[0], vertex[1], vertex[2]]);
        normals.push([vertex[3], vertex[4], vertex[5]]);
        occlusion.push(vertex[6]);
        if keep_rest {
            rest.extend_from_slice(&vertex[..6]);
        }
        if chunk.textured {
            uvs.push([vertex[7], vertex[8]]);
        }
        for (axis, value) in vertex.iter().take(3).enumerate() {
            bounds.min[axis] = bounds.min[axis].min(f64::from(*value));
            bounds.max[axis] = bounds.max[axis].max(f64::from(*value));
        }
    }
    if !keep_rest {
        pack_smooth_normals(&positions, &normals, &mut occlusion);
    }
    let positions_len = positions.len();
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(ve_render::ATTRIBUTE_OCCLUSION, occlusion);
    if chunk.textured {
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    }
    let corner_offset = corners.map_or(0, |seen| {
        let start = *seen;
        *seen += positions_len;
        start
    });
    Uploaded {
        group: chunk.group,
        region: chunk.region,
        node: chunk.node,
        name: chunk.name,
        handle: meshes.add(mesh),
        bounds,
        rest,
        corner_offset,
    }
}

/// Replaces each vertex's occlusion with one packed float: occlusion in 4 bits and the normal averaged over every vertex
/// at the same position (so a cube's corner gets the diagonal) as two 10-bit octahedral coordinates. The outline pass
/// offsets along it, which keeps the hull of hard-edged meshes closed. See `unpack_occlusion` in forward.wgsl.
fn pack_smooth_normals(positions: &[[f32; 3]], normals: &[[f32; 3]], occlusion: &mut [f32]) {
    let key = |p: &[f32; 3]| p.map(|v| (v * 1000.0).round() as i64);
    let mut sums: std::collections::HashMap<[i64; 3], [f32; 3]> = std::collections::HashMap::new();
    for (position, normal) in positions.iter().zip(normals) {
        let sum = sums.entry(key(position)).or_insert([0.0; 3]);
        for axis in 0..3 {
            sum[axis] += normal[axis];
        }
    }
    for ((position, normal), value) in positions.iter().zip(normals).zip(occlusion.iter_mut()) {
        let mut smooth = sums[&key(position)];
        let length = smooth.iter().map(|v| v * v).sum::<f32>().sqrt();
        smooth = if length > 1.0e-4 {
            smooth.map(|v| v / length)
        } else {
            *normal
        };
        let l1 = smooth.iter().map(|v| v.abs()).sum::<f32>().max(1.0e-9);
        let (mut x, mut y) = (smooth[0] / l1, smooth[1] / l1);
        if smooth[2] < 0.0 {
            let (fx, fy) = (
                (1.0 - y.abs()) * if x >= 0.0 { 1.0 } else { -1.0 },
                (1.0 - x.abs()) * if y >= 0.0 { 1.0 } else { -1.0 },
            );
            (x, y) = (fx, fy);
        }
        let quantise = |v: f32| ((v * 0.5 + 0.5) * 1021.0).round().clamp(0.0, 1021.0) as u32 + 1;
        let ao = (value.clamp(0.0, 1.0) * 15.0).round() as u32;
        *value = (ao | (quantise(x) << 4) | (quantise(y) << 14)) as f32;
    }
}
