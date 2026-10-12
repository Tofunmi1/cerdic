//! Raw FFI declarations for the C++ geometry kernel (`cpp/include/ve_kernel.h`).
//!
//! Nothing here is safe to call directly. Use the `ve-kernel` crate, which owns the audited
//! `unsafe` layer. The declarations mirror the header one to one.
#![expect(unsafe_code, reason = "this crate is the raw FFI boundary")]

use core::ffi::c_int;

/// Opaque handle to a solid owned by the C++ side. Release with [`ve_solid_free`].
#[repr(C)]
pub struct VeSolid {
    _private: [u8; 0],
}

/// Opaque cancellation context, owned by the caller.
#[repr(C)]
pub struct VeContext {
    _private: [u8; 0],
}

/// Evaluation was cancelled; the result is not reusable.
pub const VE_ERR_CANCELLED: VeStatus = 8;

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
