//! Raw tangent-space API (`MikkTSpace`); mirrors `ve_kernel.h`.
use crate::VeStatus;

// SAFETY: declarations match the linked C++ ABI.
unsafe extern "C" {
    /// `MikkTSpace` tangents: four floats per index (tangent xyz, bitangent sign), in index order.
    pub fn ve_tangents(
        positions: *const f32,
        normals: *const f32,
        uvs: *const f32,
        vertex_count: usize,
        indices: *const u32,
        index_count: usize,
        out_tangents: *mut f32,
    ) -> VeStatus;
}
