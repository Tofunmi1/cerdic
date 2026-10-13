//! Checked rigid-transform boundary.
use super::Handle;
use crate::KernelError;
use ve_kernel_sys as sys;
pub(crate) fn transform(solid: &Handle, matrix: &[f64; 12]) -> Result<Handle, KernelError> {
    let mut status = sys::VE_OK;
    // SAFETY: the solid is live, matrix has exactly twelve doubles, status is a valid output pointer.
    // The kernel validates rigidity and finiteness and retains no pointer.
    let ptr = unsafe {
        sys::ve_solid_transform(
            solid.as_ptr(),
            matrix.as_ptr(),
            matrix.len(),
            &raw mut status,
        )
    };
    Handle::from_raw(ptr, status)
}
