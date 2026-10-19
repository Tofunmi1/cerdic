//! Raw polygon-mesh API (Blender's bmesh behind the kernel); mirrors `ve_kernel.h`.
use crate::VeStatus;

/// Opaque handle to a polygon mesh produced by a bmesh operator. Release with [`ve_polymesh_free`].
#[repr(C)]
pub struct VePolymesh {
    _private: [u8; 0],
}

/// An indexed polygon mesh; borrowed buffers live through the synchronous call.
#[repr(C)]
pub struct VePolymeshInput {
    /// Size of this struct.
    pub struct_size: usize,
    /// XYZ coordinates, three floats per vertex.
    pub positions: *const f32,
    /// Number of vertices.
    pub vertex_count: usize,
    /// Corners in each face.
    pub face_sizes: *const u32,
    /// Number of faces.
    pub face_count: usize,
    /// Vertex index per corner, face after face.
    pub corners: *const u32,
    /// Number of corners.
    pub corner_count: usize,
}

/// Input of the edge bevel.
#[repr(C)]
pub struct VeBevelInput {
    /// Size of this struct.
    pub struct_size: usize,
    /// The mesh to bevel.
    pub mesh: VePolymeshInput,
    /// Vertex-index pairs, one per edge to bevel.
    pub edges: *const u32,
    /// Number of edges.
    pub edge_count: usize,
    /// Bevel size.
    pub width: f32,
    /// Segments across the bevel (1..=64).
    pub segments: i32,
    /// 0 offset, 1 width, 2 depth, 3 percent.
    pub offset_type: i32,
    /// Profile shape, 0.1..=1 (0.5 is a circular arc).
    pub profile: f32,
}

/// Input of a Blender mesh operator (`VE_MESH_OP_*` in `ve_kernel.h`).
#[repr(C)]
pub struct VeMeshOpInput {
    /// Size of this struct.
    pub struct_size: usize,
    /// The mesh to edit.
    pub mesh: VePolymeshInput,
    /// Which operator.
    pub op: i32,
    /// Selected face indices.
    pub faces: *const u32,
    /// Number of selected faces.
    pub face_count: usize,
    /// Selected edges as vertex-index pairs.
    pub edges: *const u32,
    /// Number of selected edges.
    pub edge_count: usize,
    /// Selected vertex indices.
    pub vertices: *const u32,
    /// Number of selected vertices.
    pub vertex_count: usize,
    /// Operator numbers; their meaning depends on the operator.
    pub params: [f64; 4],
    /// Operator flags; their meaning depends on the operator.
    pub flags: i32,
}

/// Input of the UV unwrap.
#[repr(C)]
pub struct VeUvInput {
    /// Size of this struct.
    pub struct_size: usize,
    /// The mesh to unwrap.
    pub mesh: VePolymeshInput,
    /// Vertex-index pairs, one per seam edge.
    pub seams: *const u32,
    /// Number of seams.
    pub seam_count: usize,
    /// 0 least-squares conformal maps, 1 angle-based flattening.
    pub method: i32,
    /// Nonzero packs the islands into the unit square.
    pub pack: i32,
    /// Nonzero lets packing rotate islands.
    pub rotate: i32,
    /// Space between packed islands, 0..=0.5 of the unit square.
    pub margin: f32,
}

/// Input of the Blender UV island packer.
#[repr(C)]
pub struct VeUvPackInput {
    /// Size of this struct.
    pub struct_size: usize,
    /// Mesh topology and positions.
    pub mesh: VePolymeshInput,
    /// Existing `(u, v)` values, two floats per corner.
    pub uvs: *const f32,
    /// Nonzero allows island rotation.
    pub rotate: i32,
    /// Nonzero keeps island scale where possible.
    pub keep_density: i32,
    /// 0 axis aligned bounds, 1 convex hull.
    pub shape: i32,
    /// Fractional margin in 0..=0.5.
    pub margin: f32,
}

// SAFETY: declarations match the linked C++ ABI.
unsafe extern "C" {
    /// Writes two floats per corner into `uvs`; counts islands that failed in `failed_islands`.
    pub fn ve_uv_unwrap(
        input: *const VeUvInput,
        uvs: *mut f32,
        uv_capacity: usize,
        failed_islands: *mut u32,
    ) -> VeStatus;
    /// Packs existing corner UVs into the unit square.
    pub fn ve_uv_pack(input: *const VeUvPackInput, uvs: *mut f32, uv_capacity: usize) -> VeStatus;
    /// Blender's easing curve (`BLI_easing`, "ease out") of `kind` (0 back, 1 bounce, 2 elastic) at `fraction`.
    pub fn ve_blender_ease(kind: i32, fraction: f32) -> f32;
    /// Bevels the given edges; null on failure with the reason in `status`.
    pub fn ve_bevel(input: *const VeBevelInput, status: *mut VeStatus) -> *mut VePolymesh;
    /// Runs a Blender mesh operator; null on failure with the reason in `status`.
    pub fn ve_mesh_op(input: *const VeMeshOpInput, status: *mut VeStatus) -> *mut VePolymesh;
    /// Number of vertices.
    pub fn ve_polymesh_num_vertices(mesh: *const VePolymesh) -> usize;
    /// Number of faces.
    pub fn ve_polymesh_num_faces(mesh: *const VePolymesh) -> usize;
    /// Number of corners.
    pub fn ve_polymesh_num_corners(mesh: *const VePolymesh) -> usize;
    /// Copies the mesh into caller buffers sized from the counts above.
    pub fn ve_polymesh_copy(
        mesh: *const VePolymesh,
        positions: *mut f32,
        position_capacity: usize,
        face_sizes: *mut u32,
        face_capacity: usize,
        corners: *mut u32,
        corner_capacity: usize,
        vertex_origin: *mut u32,
        vertex_capacity: usize,
        face_parent: *mut u32,
        parent_capacity: usize,
    ) -> VeStatus;
    /// Copies what the operator made each face (0 none, 1 inner or cap, 2 ring or side).
    pub fn ve_polymesh_copy_roles(
        mesh: *const VePolymesh,
        roles: *mut u32,
        capacity: usize,
    ) -> VeStatus;
    /// Releases a mesh.
    pub fn ve_polymesh_free(mesh: *mut VePolymesh);
}
