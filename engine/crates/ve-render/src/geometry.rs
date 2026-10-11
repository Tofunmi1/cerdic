//! Binding a batch's mesh and instance buffers, shared by every pass that draws the scene's meshes.

use std::ops::Range;

use bevy_render::mesh::allocator::MeshAllocator;
use bevy_render::mesh::{RenderMesh, RenderMeshBufferInfo};
use bevy_render::render_phase::TrackedRenderPass;
use bevy_render::render_resource::Buffer;

use crate::prepare::Batch;

/// What to draw for one batch, independent of which view it is drawn from.
pub(crate) enum DrawCall {
    Indexed {
        indices: Range<u32>,
        base_vertex: i32,
    },
    Direct {
        vertices: Range<u32>,
    },
}

impl DrawCall {
    /// Draws `instances` of the bound mesh.
    pub(crate) fn issue(&self, pass: &mut TrackedRenderPass<'_>, instances: Range<u32>) {
        match self {
            Self::Indexed {
                indices,
                base_vertex,
            } => pass.draw_indexed(indices.clone(), *base_vertex, instances),
            Self::Direct { vertices } => pass.draw(vertices.clone(), instances),
        }
    }
}

/// Binds the batch's vertex and index buffers (slot 0) and the shared instance buffer (slot 1), and
/// returns how to draw it. `None` while the mesh is not uploaded to the GPU yet.
pub(crate) fn bind_geometry<'a>(
    pass: &mut TrackedRenderPass<'a>,
    allocator: &'a MeshAllocator,
    render_mesh: &RenderMesh,
    batch: &Batch,
    instance_buffer: &'a Buffer,
) -> Option<DrawCall> {
    let vertices = allocator.mesh_vertex_slice(&batch.mesh)?;
    pass.set_vertex_buffer(0, vertices.buffer.slice(..));
    pass.set_vertex_buffer(1, instance_buffer.slice(..));
    match &render_mesh.buffer_info {
        RenderMeshBufferInfo::Indexed {
            count,
            index_format,
        } => {
            let indices = allocator.mesh_index_slice(&batch.mesh)?;
            pass.set_index_buffer(indices.buffer.slice(..), *index_format);
            Some(DrawCall::Indexed {
                indices: indices.range.start..indices.range.start + count,
                base_vertex: i32::try_from(vertices.range.start).unwrap_or(0),
            })
        }
        RenderMeshBufferInfo::NonIndexed => Some(DrawCall::Direct {
            vertices: vertices.range.start..vertices.range.end,
        }),
    }
}
