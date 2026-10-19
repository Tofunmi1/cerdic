//! Raw FFI declarations for the C++ geometry kernel (`cpp/include/ve_kernel.h`).
//!
//! Nothing here is safe to call directly. Use the `ve-kernel` crate, which owns the audited
//! `unsafe` layer. The declarations mirror the header one to one.
#![expect(unsafe_code, reason = "this crate is the raw FFI boundary")]

use core::ffi::c_int;
mod polymesh;
mod profiles;
mod tangents;
pub use polymesh::*;
pub use profiles::*;
pub use tangents::*;

/// Opaque handle to a solid owned by the C++ side. Release with [`ve_solid_free`].
#[repr(C)]
pub struct VeSolid {
    _private: [u8; 0],
}

/// Opaque handle to a subdivided polygon mesh.
#[repr(C)]
pub struct VeSubdivision {
    _private: [u8; 0],
}

/// Versioned Catmull-Clark input; borrowed buffers live through the synchronous call.
#[repr(C)]
pub struct VeSubdivideInput {
    /// Size of this struct; must include every field used by the current ABI.
    pub struct_size: usize,
    /// XYZ point coordinates.
    pub positions: *const f32,
    /// Number of points.
    pub vertex_count: usize,
    /// Number of corners in each face.
    pub face_sizes: *const u32,
    /// Number of faces.
    pub face_count: usize,
    /// Point index for each face corner.
    pub corners: *const u32,
    /// Number of corners.
    pub corner_count: usize,
    /// Endpoint pairs for sharp cage edges.
    pub crease_edges: *const u32,
    /// Sharpness value for each edge.
    pub crease_weights: *const f32,
    /// Number of sharp edges.
    pub crease_count: usize,
    /// Cage point indices with corner sharpness.
    pub corner_vertices: *const u32,
    /// Sharpness value for each corner point.
    pub corner_weights: *const f32,
    /// Number of sharp corner points.
    pub corner_count_values: usize,
    /// Reserved face indices for holes; currently unsupported by the adapter.
    pub hole_faces: *const u32,
    /// Number of hole faces.
    pub hole_count: usize,
    /// Uniform refinement level count.
    pub levels: u32,
    /// Vertex boundary interpolation rule.
    pub boundary_rule: c_int,
    /// Edge crease decay method.
    pub crease_method: c_int,
    /// Interleaved point primvar values.
    pub point_values: *const f32,
    /// Number of interleaved point primvar channels.
    pub point_channels: usize,
    /// Interleaved face-varying values.
    pub fvar_values: *const f32,
    /// Number of unique face-varying values.
    pub fvar_value_count: usize,
    /// Scalars per face-varying value (2 for UV).
    pub fvar_width: u32,
    /// Face-varying value index for each cage corner.
    pub fvar_corner_values: *const u32,
    /// Borrowed cancellation context.
    pub context: *const VeContext,
}

/// Opaque cancellation context, owned by the caller.
#[repr(C)]
pub struct VeContext {
    _private: [u8; 0],
}

/// One deterministic spatial-field tape instruction.
#[repr(C)]
pub struct VeFieldInstruction {
    /// Opcode from the `VE_FIELD_*` constants.
    pub opcode: u32,
    /// First input register.
    pub input_a: u32,
    /// Second input register.
    pub input_b: u32,
    /// Opcode-specific constants.
    pub x: f64,
    /// Opcode-specific constants.
    pub y: f64,
    /// Opcode-specific constants.
    pub z: f64,
    /// Opcode-specific constants.
    pub w: f64,
}

/// Bounded field meshing input; its slices stay alive for the synchronous call.
#[repr(C)]
pub struct VeFieldInput {
    /// Size of the struct, for ABI versioning.
    pub struct_size: usize,
    /// Register instructions in topological order.
    pub instructions: *const VeFieldInstruction,
    /// Number of instructions.
    pub instruction_count: usize,
    /// Axis bounds in min.xyz, max.xyz order.
    pub bounds: [f64; 6],
    /// Requested voxel edge length.
    pub edge_length: f64,
    /// Caller-side cell cap, also checked by C++.
    pub max_cells: usize,
    /// Caller-side instruction-times-cell cap, also checked by C++.
    pub max_operations: usize,
    /// Borrowed cancellation context.
    pub context: *mut VeContext,
}

/// Evaluation was cancelled; the result is not reusable.
pub const VE_ERR_CANCELLED: VeStatus = 8;
/// Bounded subdivision attribute channel storage was exceeded.
pub const VE_ERR_ATTRIBUTE_LIMIT: VeStatus = 9;
/// The field is positive on the sampled bounds boundary.
pub const VE_ERR_FIELD_CLIPPED: VeStatus = 10;
/// The field did not produce any surface.
pub const VE_ERR_FIELD_EMPTY: VeStatus = 11;
/// The grid, padded voxel storage or instruction work exceeded its cap.
pub const VE_ERR_FIELD_BUDGET: VeStatus = 12;
/// A Blender operator ran and declined the input.
pub const VE_ERR_OP_REFUSED: VeStatus = 13;

/// Status code returned through `ve_status*` out-parameters (see the `VE_*` constants).
pub type VeStatus = i32;

/// Success.
pub const VE_OK: VeStatus = 0;
/// An argument was null, non-finite, out of range or otherwise invalid.
pub const VE_ERR_INVALID_ARG: VeStatus = 1;
/// The input mesh or the result is not a closed manifold.
pub const VE_ERR_NOT_MANIFOLD: VeStatus = 2;
/// A vertex coordinate was NaN or infinite.
pub const VE_ERR_NON_FINITE: VeStatus = 3;
/// An index referred to a vertex that does not exist.
pub const VE_ERR_OUT_OF_BOUNDS: VeStatus = 4;
/// The result exceeds representable size.
pub const VE_ERR_TOO_LARGE: VeStatus = 5;
/// A caller-provided output buffer was too small.
pub const VE_ERR_BUFFER_TOO_SMALL: VeStatus = 6;
/// Unexpected internal failure (for example allocation).
pub const VE_ERR_INTERNAL: VeStatus = 7;

/// Upper bound on circular segments accepted by the kernel.
pub const VE_MAX_SEGMENTS: u32 = 4096;

// SAFETY (all declarations): signatures match `cpp/include/ve_kernel.h`; the library is linked by
// this crate's build script.
unsafe extern "C" {
    /// Refines a polygon cage and returns an owned result handle.
    pub fn ve_subdivide_catmark(
        input: *const VeSubdivideInput,
        status: *mut VeStatus,
    ) -> *mut VeSubdivision;
    /// Number of refined vertices.
    pub fn ve_subdivision_num_vertices(subdivision: *const VeSubdivision) -> usize;
    /// Number of refined faces.
    pub fn ve_subdivision_num_faces(subdivision: *const VeSubdivision) -> usize;
    /// Number of refined corners.
    pub fn ve_subdivision_num_corners(subdivision: *const VeSubdivision) -> usize;
    /// Number of interleaved point channels on the output vertices.
    pub fn ve_subdivision_num_point_channels(subdivision: *const VeSubdivision) -> usize;
    /// Number of unique face-varying values on the refined mesh.
    pub fn ve_subdivision_num_fvar_values(subdivision: *const VeSubdivision) -> usize;
    /// Copies the complete refined result into caller-owned buffers.
    pub fn ve_subdivision_copy(
        subdivision: *const VeSubdivision,
        positions: *mut f32,
        positions_capacity: usize,
        face_sizes: *mut u32,
        face_capacity: usize,
        corners: *mut u32,
        corner_capacity: usize,
        vertex_origin_kind: *mut u32,
        vertex_origin_index: *mut u32,
        vertex_origin_capacity: usize,
        face_parent: *mut u32,
        face_parent_capacity: usize,
    ) -> VeStatus;
    /// Copies point and face-varying channels into caller-owned buffers.
    pub fn ve_subdivision_copy_channels(
        subdivision: *const VeSubdivision,
        point_values: *mut f32,
        point_values_capacity: usize,
        fvar_values: *mut f32,
        fvar_values_capacity: usize,
        fvar_corner_values: *mut u32,
        fvar_corner_capacity: usize,
    ) -> VeStatus;
    /// Releases an owned refined result.
    pub fn ve_subdivision_free(subdivision: *mut VeSubdivision);
    /// Row-major proper rigid 3x4 transform, checked in C++.
    pub fn ve_solid_transform(
        s: *const VeSolid,
        matrix: *const f64,
        count: usize,
        status: *mut VeStatus,
    ) -> *mut VeSolid;
    /// Deferred batch union evaluated with one context, 2..64 borrowed handles.
    pub fn ve_solid_batch_union_context(
        solids: *const *const VeSolid,
        count: usize,
        context: *const VeContext,
        status: *mut VeStatus,
    ) -> *mut VeSolid;
    /// Creates a fresh context.
    pub fn ve_context_new() -> *mut VeContext;
    /// Releases a context after its last borrowed use.
    pub fn ve_context_free(context: *mut VeContext);
    /// Permanently requests cancellation; thread-safe in Manifold.
    pub fn ve_context_cancel(context: *mut VeContext);
    /// Whether cancellation has been requested.
    pub fn ve_context_cancelled(context: *const VeContext) -> i32;
    /// Boolean evaluated with cooperative cancellation (0 union, 1 difference, 2 intersection).
    pub fn ve_solid_boolean_context(
        a: *const VeSolid,
        b: *const VeSolid,
        operation: i32,
        context: *const VeContext,
        status: *mut VeStatus,
    ) -> *mut VeSolid;
    /// Axis-aligned box, optionally centred on the origin.
    pub fn ve_solid_cube(
        x: f64,
        y: f64,
        z: f64,
        center: i32,
        status: *mut VeStatus,
    ) -> *mut VeSolid;
    /// UV sphere.
    pub fn ve_solid_sphere(radius: f64, segments: i32, status: *mut VeStatus) -> *mut VeSolid;
    /// Cylinder or cone frustum along Y.
    pub fn ve_solid_cylinder(
        height: f64,
        radius_low: f64,
        radius_high: f64,
        segments: i32,
        center: i32,
        status: *mut VeStatus,
    ) -> *mut VeSolid;
    /// Builds a solid from an indexed triangle mesh.
    pub fn ve_solid_from_mesh(
        positions: *const f32,
        vertex_count: usize,
        indices: *const u32,
        index_count: usize,
        status: *mut VeStatus,
    ) -> *mut VeSolid;
    /// Samples a deterministic field tape and returns a closed solid.
    pub fn ve_solid_from_field(
        input: *const VeFieldInput,
        clipped_point: *mut f64,
        status: *mut VeStatus,
    ) -> *mut VeSolid;
    /// Evaluates one point with the C++ field tape interpreter.
    pub fn ve_field_sample(
        instructions: *const VeFieldInstruction,
        instruction_count: usize,
        point: *const f64,
        out_value: *mut f64,
    ) -> VeStatus;
    /// Reserves `count` fresh, consecutive origin ids and returns the first.
    pub fn ve_reserve_ids(count: u32) -> u32;
    /// Builds a solid from a mesh whose triangle runs carry ascending origin ids.
    pub fn ve_solid_from_mesh_runs(
        positions: *const f32,
        vertex_count: usize,
        indices: *const u32,
        index_count: usize,
        run_first_triangle: *const usize,
        run_ids: *const u32,
        run_count: usize,
        status: *mut VeStatus,
    ) -> *mut VeSolid;
    /// Number of triangle runs in the mesh `ve_solid_copy_mesh` copies.
    pub fn ve_solid_num_runs(s: *const VeSolid) -> usize;
    /// Copies the first triangle and origin id of each run.
    pub fn ve_solid_copy_runs(
        s: *const VeSolid,
        first_triangle: *mut usize,
        ids: *mut u32,
        capacity: usize,
    ) -> VeStatus;
    /// Boolean union.
    pub fn ve_solid_union(
        a: *const VeSolid,
        b: *const VeSolid,
        status: *mut VeStatus,
    ) -> *mut VeSolid;
    /// Boolean difference (`a` minus `b`).
    pub fn ve_solid_difference(
        a: *const VeSolid,
        b: *const VeSolid,
        status: *mut VeStatus,
    ) -> *mut VeSolid;
    /// Boolean intersection.
    pub fn ve_solid_intersect(
        a: *const VeSolid,
        b: *const VeSolid,
        status: *mut VeStatus,
    ) -> *mut VeSolid;
    /// Inset and outset by a sphere to round solid edges.
    pub fn ve_solid_round(
        solid: *const VeSolid,
        radius: f64,
        segments: i32,
        status: *mut VeStatus,
    ) -> *mut VeSolid;
    /// Inset and outset by an octahedron to chamfer solid edges with a flat facet.
    pub fn ve_solid_chamfer(
        solid: *const VeSolid,
        width: f64,
        status: *mut VeStatus,
    ) -> *mut VeSolid;
    /// Translation.
    pub fn ve_solid_translate(
        s: *const VeSolid,
        x: f64,
        y: f64,
        z: f64,
        status: *mut VeStatus,
    ) -> *mut VeSolid;
    /// Rotation, Euler angles in degrees.
    pub fn ve_solid_rotate(
        s: *const VeSolid,
        x_deg: f64,
        y_deg: f64,
        z_deg: f64,
        status: *mut VeStatus,
    ) -> *mut VeSolid;
    /// Non-uniform scale.
    pub fn ve_solid_scale(
        s: *const VeSolid,
        x: f64,
        y: f64,
        z: f64,
        status: *mut VeStatus,
    ) -> *mut VeSolid;
    /// Volume in cubic metres.
    pub fn ve_solid_volume(s: *const VeSolid) -> f64;
    /// Surface area in square metres.
    pub fn ve_solid_surface_area(s: *const VeSolid) -> f64;
    /// Number of vertices.
    pub fn ve_solid_num_verts(s: *const VeSolid) -> usize;
    /// Number of triangles.
    pub fn ve_solid_num_tris(s: *const VeSolid) -> usize;
    /// Number of connected components.
    pub fn ve_solid_num_components(s: *const VeSolid) -> usize;
    /// Non-zero if the solid has no geometry.
    pub fn ve_solid_is_empty(s: *const VeSolid) -> c_int;
    /// Writes `{min.xyz, max.xyz}` into `out`.
    pub fn ve_solid_bounds(s: *const VeSolid, out: *mut f64) -> VeStatus;
    /// Copies the render mesh into caller-owned buffers (capacities in elements).
    pub fn ve_solid_copy_mesh(
        s: *const VeSolid,
        positions: *mut f32,
        positions_capacity: usize,
        indices: *mut u32,
        indices_capacity: usize,
    ) -> VeStatus;
    /// Simplifies an indexed triangle mesh.
    pub fn ve_simplify(
        positions: *const f32,
        vertex_count: usize,
        indices: *const u32,
        index_count: usize,
        target_ratio: f64,
        target_error: f64,
        out_indices: *mut u32,
        out_capacity: usize,
        out_count: *mut usize,
    ) -> VeStatus;
    /// Releases a solid. Null is allowed.
    pub fn ve_solid_free(s: *mut VeSolid);
}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
unsafe extern "C" {
    /// Runs the C++ static constructors. A `wasm32-unknown-unknown` module is a library with no
    /// start routine, so nothing else does; it must be called once before any kernel function.
    pub fn __wasm_call_ctors();
}
