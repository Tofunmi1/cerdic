//! The audited `unsafe` layer: every call into the C++ kernel goes through this module.
//!
//! Invariants relied on throughout:
//! - A [`Handle`] wraps a non-null pointer returned by the kernel and owned exclusively by it, so it is
//!   freed exactly once, in `Drop`.
//! - The kernel never retains pointers passed to it; all slices only need to outlive the call.
//! - Kernel objects are immutable after construction, so moving a `Handle` between threads is
//!   sound (`Send`); concurrent shared use is not promised, so there is no `Sync`.

use core::ptr::NonNull;

use ve_kernel_sys as sys;

use crate::KernelError;

pub(crate) mod context;
pub(crate) mod profile;

/// Runs the C++ static constructors once on `wasm32-unknown-unknown`, where nothing else does.
/// A no-op on every other target (native and WASI start the C++ runtime themselves).
fn ensure_initialized() {
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    {
        static INIT: std::sync::Once = std::sync::Once::new();
        // SAFETY: runs exactly once, before any other kernel call, on the only thread.
        INIT.call_once(|| unsafe { sys::__wasm_call_ctors() });
    }
}

/// Owning handle to a kernel solid.
pub(crate) struct Handle(NonNull<sys::VeSolid>);

// SAFETY: the C++ solid is immutable after construction and has no thread-affine state.
unsafe impl Send for Handle {}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: `self.0` came from the kernel, is non-null and is freed only here.
        unsafe { sys::ve_solid_free(self.0.as_ptr()) };
    }
}

impl Handle {
    fn from_raw(ptr: *mut sys::VeSolid, status: sys::VeStatus) -> Result<Self, KernelError> {
        match (NonNull::new(ptr), status) {
            (Some(handle), sys::VE_OK) => Ok(Self(handle)),
            (Some(handle), code) => {
                // SAFETY: the kernel returned a live handle; release it instead of leaking.
                unsafe { sys::ve_solid_free(handle.as_ptr()) };
                Err(KernelError::from_status(code))
            }
            (None, sys::VE_OK) => Err(KernelError::Internal(sys::VE_ERR_INTERNAL)),
            (None, code) => Err(KernelError::from_status(code)),
        }
    }

    fn as_ptr(&self) -> *const sys::VeSolid {
        self.0.as_ptr()
    }
}

pub(crate) fn cube(size: [f64; 3], center: bool) -> Result<Handle, KernelError> {
    ensure_initialized();
    let mut status = sys::VE_OK;
    // SAFETY: `status` is a valid out-pointer for the duration of the call.
    let ptr = unsafe {
        sys::ve_solid_cube(
            size[0],
            size[1],
            size[2],
            i32::from(center),
            &raw mut status,
        )
    };
    Handle::from_raw(ptr, status)
}

pub(crate) fn sphere(radius: f64, segments: i32) -> Result<Handle, KernelError> {
    ensure_initialized();
    let mut status = sys::VE_OK;
    // SAFETY: `status` is a valid out-pointer for the duration of the call.
    let ptr = unsafe { sys::ve_solid_sphere(radius, segments, &raw mut status) };
    Handle::from_raw(ptr, status)
}

pub(crate) fn cylinder(
    height: f64,
    radius_low: f64,
    radius_high: f64,
    segments: i32,
    center: bool,
) -> Result<Handle, KernelError> {
    ensure_initialized();
    let mut status = sys::VE_OK;
    // SAFETY: `status` is a valid out-pointer for the duration of the call.
    let ptr = unsafe {
        sys::ve_solid_cylinder(
            height,
            radius_low,
            radius_high,
            segments,
            i32::from(center),
            &raw mut status,
        )
    };
    Handle::from_raw(ptr, status)
}

pub(crate) fn from_mesh(positions: &[[f32; 3]], indices: &[u32]) -> Result<Handle, KernelError> {
    ensure_initialized();
    let flat = positions.as_flattened();
    let mut status = sys::VE_OK;
    // SAFETY: `flat` holds `3 * positions.len()` floats and `indices` holds `indices.len()` entries; the
    // kernel copies both before returning and validates ranges itself.
    let ptr = unsafe {
        sys::ve_solid_from_mesh(
            flat.as_ptr(),
            positions.len(),
            indices.as_ptr(),
            indices.len(),
            &raw mut status,
        )
    };
    Handle::from_raw(ptr, status)
}

type BinaryOp = unsafe extern "C" fn(
    *const sys::VeSolid,
    *const sys::VeSolid,
    *mut sys::VeStatus,
) -> *mut sys::VeSolid;

fn binary(op: BinaryOp, a: &Handle, b: &Handle) -> Result<Handle, KernelError> {
    let mut status = sys::VE_OK;
    // SAFETY: both handles are live (borrowed) and `status` is a valid out-pointer.
    let ptr = unsafe { op(a.as_ptr(), b.as_ptr(), &raw mut status) };
    Handle::from_raw(ptr, status)
}

pub(crate) fn union(a: &Handle, b: &Handle) -> Result<Handle, KernelError> {
    binary(sys::ve_solid_union, a, b)
}

pub(crate) fn difference(a: &Handle, b: &Handle) -> Result<Handle, KernelError> {
    binary(sys::ve_solid_difference, a, b)
}

pub(crate) fn intersect(a: &Handle, b: &Handle) -> Result<Handle, KernelError> {
    binary(sys::ve_solid_intersect, a, b)
}

type TransformOp = unsafe extern "C" fn(
    *const sys::VeSolid,
    f64,
    f64,
    f64,
    *mut sys::VeStatus,
) -> *mut sys::VeSolid;

fn transform(op: TransformOp, s: &Handle, v: [f64; 3]) -> Result<Handle, KernelError> {
    let mut status = sys::VE_OK;
    // SAFETY: the handle is live (borrowed) and `status` is a valid out-pointer.
    let ptr = unsafe { op(s.as_ptr(), v[0], v[1], v[2], &raw mut status) };
    Handle::from_raw(ptr, status)
}

pub(crate) fn translate(s: &Handle, by: [f64; 3]) -> Result<Handle, KernelError> {
    transform(sys::ve_solid_translate, s, by)
}

pub(crate) fn rotate(s: &Handle, degrees: [f64; 3]) -> Result<Handle, KernelError> {
    transform(sys::ve_solid_rotate, s, degrees)
}

pub(crate) fn scale(s: &Handle, by: [f64; 3]) -> Result<Handle, KernelError> {
    transform(sys::ve_solid_scale, s, by)
}

pub(crate) fn volume(s: &Handle) -> f64 {
    // SAFETY: the handle is live (borrowed).
    unsafe { sys::ve_solid_volume(s.as_ptr()) }
}

pub(crate) fn surface_area(s: &Handle) -> f64 {
    // SAFETY: the handle is live (borrowed).
    unsafe { sys::ve_solid_surface_area(s.as_ptr()) }
}

pub(crate) fn num_verts(s: &Handle) -> usize {
    // SAFETY: the handle is live (borrowed).
    unsafe { sys::ve_solid_num_verts(s.as_ptr()) }
}

pub(crate) fn num_tris(s: &Handle) -> usize {
    // SAFETY: the handle is live (borrowed).
    unsafe { sys::ve_solid_num_tris(s.as_ptr()) }
}

pub(crate) fn num_components(s: &Handle) -> usize {
    // SAFETY: the handle is live (borrowed).
    unsafe { sys::ve_solid_num_components(s.as_ptr()) }
}

pub(crate) fn is_empty(s: &Handle) -> bool {
    // SAFETY: the handle is live (borrowed).
    unsafe { sys::ve_solid_is_empty(s.as_ptr()) != 0 }
}

pub(crate) fn bounds(s: &Handle) -> Result<[f64; 6], KernelError> {
    let mut out = [0.0_f64; 6];
    // SAFETY: the handle is live and `out` has room for the six doubles the kernel writes.
    let status = unsafe { sys::ve_solid_bounds(s.as_ptr(), out.as_mut_ptr()) };
    if status == sys::VE_OK {
        Ok(out)
    } else {
        Err(KernelError::from_status(status))
    }
}

pub(crate) fn copy_mesh(
    s: &Handle,
    positions: &mut [[f32; 3]],
    indices: &mut [u32],
) -> Result<(), KernelError> {
    let flat = positions.as_flattened_mut();
    // SAFETY: the handle is live; capacities passed are exactly the lengths of the buffers.
    let status = unsafe {
        sys::ve_solid_copy_mesh(
            s.as_ptr(),
            flat.as_mut_ptr(),
            flat.len(),
            indices.as_mut_ptr(),
            indices.len(),
        )
    };
    if status == sys::VE_OK {
        Ok(())
    } else {
        Err(KernelError::from_status(status))
    }
}

pub(crate) fn simplify(
    positions: &[[f32; 3]],
    indices: &[u32],
    target_ratio: f64,
    target_error: f64,
) -> Result<Vec<u32>, KernelError> {
    ensure_initialized();
    let flat = positions.as_flattened();
    let mut out = vec![0_u32; indices.len()];
    let mut count = 0_usize;
    // SAFETY: input slices are valid for their lengths; `out` has `indices.len()` slots, which is the
    // capacity passed; `count` is a valid out-pointer.
    let status = unsafe {
        sys::ve_simplify(
            flat.as_ptr(),
            positions.len(),
            indices.as_ptr(),
            indices.len(),
            target_ratio,
            target_error,
            out.as_mut_ptr(),
            out.len(),
            &raw mut count,
        )
    };
    if status != sys::VE_OK {
        return Err(KernelError::from_status(status));
    }
    out.truncate(count);
    Ok(out)
}
