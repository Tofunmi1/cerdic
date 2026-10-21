//! Mesh and texture output.
use super::HeightGrid;
use crate::num::{cell_f32, cell_u32, texture_u16, weight_u8};

/// A triangle mesh as flat arrays.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MeshOut {
    /// Vertex positions in world metres.
    pub positions: Vec<[f32; 3]>,
    /// Unit vertex normals.
    pub normals: Vec<[f32; 3]>,
    /// UVs across the grid.
    pub uvs: Vec<[f32; 2]>,
    /// Triangle indices, counter-clockwise seen from +y.
    pub indices: Vec<u32>,
}

/// Meshes every `stride`th cell and includes the far edge.
#[must_use]
pub fn to_mesh(grid: &HeightGrid, stride: usize) -> MeshOut {
    let stride = stride.max(1);
    let xs = sample_indices(grid.width(), stride);
    let zs = sample_indices(grid.depth(), stride);
    let mut mesh = MeshOut::default();
    for &z in &zs {
        for &x in &xs {
            mesh.positions.push([
                grid.world_x(x),
                grid.heights()[z * grid.width() + x],
                grid.world_z(z),
            ]);
            mesh.normals
                .push(grid.normal(grid.world_x(x), grid.world_z(z)));
            mesh.uvs.push([
                cell_f32(x) / cell_f32(grid.width() - 1),
                cell_f32(z) / cell_f32(grid.depth() - 1),
            ]);
        }
    }
    for z in 0..zs.len().saturating_sub(1) {
        for x in 0..xs.len().saturating_sub(1) {
            let a = cell_u32(z * xs.len() + x);
            let b = a + 1;
            let c = a + cell_u32(xs.len());
            let d = c + 1;
            // +y winding in x/z coordinates.
            mesh.indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    mesh
}

fn sample_indices(length: usize, stride: usize) -> Vec<usize> {
    let mut out: Vec<usize> = (0..length).step_by(stride).collect();
    if out.last().copied() != Some(length - 1) {
        out.push(length - 1);
    }
    out
}

/// Quantises heights to 16 bits over `min..=max`.
#[must_use]
pub fn to_height_texture(grid: &HeightGrid, min: f32, max: f32) -> Vec<u16> {
    grid.heights()
        .iter()
        .map(|h| {
            let t = if max > min {
                ((*h - min) / (max - min)).clamp(0.0, 1.0)
            } else {
                0.0
            };
            texture_u16((t * 65_535.0).round())
        })
        .collect()
}

/// Packs up to four mask layers into normalized RGBA8 weights.
#[must_use]
pub fn pack_splat(layers: &[&[f32]]) -> Vec<u8> {
    let count = layers.iter().map(|layer| layer.len()).min().unwrap_or(0);
    let mut result = vec![0_u8; count * 4];
    for i in 0..count {
        let weights: [f32; 4] = core::array::from_fn(|channel| {
            layers.get(channel).map_or(0.0, |layer| layer[i].max(0.0))
        });
        let sum: f32 = weights.iter().sum();
        if sum > 0.0 {
            let mut used = 0_u32;
            for channel in 0..4 {
                if channel == 3 {
                    result[i * 4 + channel] = u8::try_from(255 - used).unwrap_or(0);
                } else {
                    let value = weight_u8((weights[channel] / sum * 255.0).round());
                    result[i * 4 + channel] = value;
                    used += u32::from(value);
                }
            }
        }
    }
    result
}
