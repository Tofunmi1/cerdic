//! Audited FFI for the OpenSubdiv-backed Catmull-Clark adapter.
use super::super::SubdivisionChannels;
use crate::{ExecutionContext, KernelError};
use core::ptr::NonNull;
use ve_kernel_sys as sys;

pub(crate) struct Subdivision(NonNull<sys::VeSubdivision>);

impl Drop for Subdivision {
    fn drop(&mut self) {
        // SAFETY: this is the unique owning pointer returned by the C ABI.
        unsafe { sys::ve_subdivision_free(self.0.as_ptr()) };
    }
}

pub(crate) struct RawOutput {
    pub positions: Vec<[f32; 3]>,
    pub face_sizes: Vec<u32>,
    pub corners: Vec<u32>,
    pub vertex_origin_kind: Vec<u32>,
    pub vertex_origin_index: Vec<u32>,
    pub face_parent: Vec<u32>,
    pub point_values: Vec<f32>,
    pub point_channels: usize,
    pub fvar_values: Vec<[f32; 2]>,
    pub fvar_corner_values: Vec<u32>,
}

pub(crate) fn catmark(
    positions: &[[f32; 3]],
    face_sizes: &[u32],
    corners: &[u32],
    crease_edges: &[[u32; 2]],
    crease_weights: &[f32],
    corner_vertices: &[u32],
    corner_weights: &[f32],
    channels: SubdivisionChannels<'_>,
    levels: u32,
    boundary_rule: i32,
    crease_method: i32,
    context: &ExecutionContext,
) -> Result<RawOutput, KernelError> {
    if crease_edges.len() != crease_weights.len() || corner_vertices.len() != corner_weights.len() {
        return Err(KernelError::InvalidArgument("subdivision sharpness values"));
    }
    let flat_positions = positions.as_flattened();
    let flat_creases = crease_edges.as_flattened();
    let flat_fvar_values = channels.fvar_values.as_flattened();
    let input = sys::VeSubdivideInput {
        struct_size: core::mem::size_of::<sys::VeSubdivideInput>(),
        positions: flat_positions.as_ptr(),
        vertex_count: positions.len(),
        face_sizes: face_sizes.as_ptr(),
        face_count: face_sizes.len(),
        corners: corners.as_ptr(),
        corner_count: corners.len(),
        crease_edges: optional_ptr(flat_creases),
        crease_weights: optional_ptr(crease_weights),
        crease_count: crease_edges.len(),
        corner_vertices: optional_ptr(corner_vertices),
        corner_weights: optional_ptr(corner_weights),
        corner_count_values: corner_vertices.len(),
        hole_faces: core::ptr::null(),
        hole_count: 0,
        levels,
        boundary_rule,
        crease_method,
        point_values: optional_ptr(channels.point_values),
        point_channels: channels.point_channels,
        fvar_values: optional_ptr(flat_fvar_values),
        fvar_value_count: channels.fvar_values.len(),
        fvar_width: if channels.fvar_values.is_empty() {
            0
        } else {
            2
        },
        fvar_corner_values: optional_ptr(channels.fvar_corner_values),
        context: context.0.as_ptr(),
    };
    let handle = call_catmark(&input)?;
    copy_output(&handle)
}

fn call_catmark(input: &sys::VeSubdivideInput) -> Result<Subdivision, KernelError> {
    let mut status = sys::VE_OK;
    // SAFETY: every borrowed input array and context outlive this synchronous call; status is writable.
    let raw = unsafe { sys::ve_subdivide_catmark(core::ptr::from_ref(input), &raw mut status) };
    match (NonNull::new(raw), status) {
        (Some(ptr), sys::VE_OK) => Ok(Subdivision(ptr)),
        (Some(ptr), code) => {
            // SAFETY: C returned an owned handle despite failure; dispose it.
            unsafe { sys::ve_subdivision_free(ptr.as_ptr()) };
            Err(KernelError::from_status(code))
        }
        (None, sys::VE_OK) => Err(KernelError::Internal(sys::VE_ERR_INTERNAL)),
        (None, code) => Err(KernelError::from_status(code)),
    }
}

fn copy_output(handle: &Subdivision) -> Result<RawOutput, KernelError> {
    // SAFETY: handle remains live until copied data owns all output buffers.
    let vertex_count = unsafe { sys::ve_subdivision_num_vertices(handle.0.as_ptr()) };
    // SAFETY: same live immutable handle.
    let face_count = unsafe { sys::ve_subdivision_num_faces(handle.0.as_ptr()) };
    // SAFETY: same live immutable handle.
    let corner_count = unsafe { sys::ve_subdivision_num_corners(handle.0.as_ptr()) };
    // SAFETY: same live immutable handle.
    let point_channels = unsafe { sys::ve_subdivision_num_point_channels(handle.0.as_ptr()) };
    // SAFETY: same live immutable handle.
    let fvar_value_count = unsafe { sys::ve_subdivision_num_fvar_values(handle.0.as_ptr()) };
    let mut raw_positions = vec![0.0; vertex_count.saturating_mul(3)];
    let mut output = RawOutput {
        positions: vec![[0.0; 3]; vertex_count],
        face_sizes: vec![0; face_count],
        corners: vec![0; corner_count],
        vertex_origin_kind: vec![0; vertex_count],
        vertex_origin_index: vec![0; vertex_count],
        face_parent: vec![0; face_count],
        point_values: vec![0.0; vertex_count.saturating_mul(point_channels)],
        point_channels,
        fvar_values: vec![[0.0; 2]; fvar_value_count],
        fvar_corner_values: vec![
            0;
            if fvar_value_count == 0 {
                0
            } else {
                corner_count
            }
        ],
    };
    // SAFETY: capacities exactly match queried counts from the same live handle.
    let status = unsafe {
        sys::ve_subdivision_copy(
            handle.0.as_ptr(),
            raw_positions.as_mut_ptr(),
            raw_positions.len(),
            output.face_sizes.as_mut_ptr(),
            output.face_sizes.len(),
            output.corners.as_mut_ptr(),
            output.corners.len(),
            output.vertex_origin_kind.as_mut_ptr(),
            output.vertex_origin_index.as_mut_ptr(),
            vertex_count,
            output.face_parent.as_mut_ptr(),
            output.face_parent.len(),
        )
    };
    if status != sys::VE_OK {
        return Err(KernelError::from_status(status));
    }
    let mut flat_fvar_values = vec![0.0; fvar_value_count.saturating_mul(2)];
    // SAFETY: capacities are the exact sizes reported by this live handle; null is used only for
    // empty buffers, which the C implementation accepts for absent channels.
    let status = unsafe {
        sys::ve_subdivision_copy_channels(
            handle.0.as_ptr(),
            optional_mut_ptr(&mut output.point_values),
            output.point_values.len(),
            optional_mut_ptr(&mut flat_fvar_values),
            flat_fvar_values.len(),
            optional_mut_ptr(&mut output.fvar_corner_values),
            output.fvar_corner_values.len(),
        )
    };
    if status != sys::VE_OK {
        return Err(KernelError::from_status(status));
    }
    for (uv, values) in output
        .fvar_values
        .iter_mut()
        .zip(flat_fvar_values.chunks_exact(2))
    {
        uv.copy_from_slice(values);
    }
    for (point, xyz) in output
        .positions
        .iter_mut()
        .zip(raw_positions.chunks_exact(3))
    {
        point.copy_from_slice(xyz);
    }
    Ok(output)
}

fn optional_ptr<T>(values: &[T]) -> *const T {
    if values.is_empty() {
        core::ptr::null()
    } else {
        values.as_ptr()
    }
}

fn optional_mut_ptr<T>(values: &mut [T]) -> *mut T {
    if values.is_empty() {
        core::ptr::null_mut()
    } else {
        values.as_mut_ptr()
    }
}
