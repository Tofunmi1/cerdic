//! Audited profile ownership and copy boundaries.
use super::{Handle, ensure_initialized};
use crate::KernelError;
use core::ptr::NonNull;
use ve_kernel_sys as sys;

pub(crate) struct ProfileHandle(NonNull<sys::VeProfile>);

// SAFETY: immutable cross sections have no thread affinity; shared access is not promised.
unsafe impl Send for ProfileHandle {}

impl Drop for ProfileHandle {
    fn drop(&mut self) {
        // SAFETY: this exclusively owned, non-null pointer is freed once here.
        unsafe { sys::ve_profile_free(self.0.as_ptr()) };
    }
}

impl ProfileHandle {
    fn from_raw(ptr: *mut sys::VeProfile, status: sys::VeStatus) -> Result<Self, KernelError> {
        let owned = NonNull::new(ptr).map(Self);
        if status != sys::VE_OK {
            return Err(KernelError::from_status(status));
        }
        owned.ok_or(KernelError::Internal(sys::VE_ERR_INTERNAL))
    }
}

pub(crate) fn new(points: &[[f64; 2]], offsets: &[usize]) -> Result<ProfileHandle, KernelError> {
    ensure_initialized();
    let mut status = sys::VE_OK;
    // SAFETY: buffers have the exact advertised lengths; the backend copies before returning.
    let ptr = unsafe {
        sys::ve_profile_new(
            points.as_flattened().as_ptr(),
            points.len(),
            offsets.as_ptr(),
            offsets.len() - 1,
            &raw mut status,
        )
    };
    ProfileHandle::from_raw(ptr, status)
}

pub(crate) fn area(profile: &ProfileHandle) -> f64 {
    // SAFETY: borrowed handle is live.
    unsafe { sys::ve_profile_area(profile.0.as_ptr()) }
}

pub(crate) fn loops(profile: &ProfileHandle) -> Result<Vec<Vec<[f64; 2]>>, KernelError> {
    let (mut count, mut loop_count) = (0, 0);
    // SAFETY: live borrowed handle and valid output pointers.
    let status =
        unsafe { sys::ve_profile_counts(profile.0.as_ptr(), &raw mut count, &raw mut loop_count) };
    if status != sys::VE_OK {
        return Err(KernelError::from_status(status));
    }
    if count > 16384 || loop_count > 64 {
        return Err(KernelError::Internal(sys::VE_ERR_TOO_LARGE));
    }
    let mut points = vec![[0.0; 2]; count];
    let mut offsets = vec![0; loop_count + 1];
    // SAFETY: writable capacities equal the actual buffer lengths; no pointer is retained.
    let status = unsafe {
        sys::ve_profile_copy(
            profile.0.as_ptr(),
            points.as_flattened_mut().as_mut_ptr(),
            count * 2,
            offsets.as_mut_ptr(),
            offsets.len(),
        )
    };
    if status != sys::VE_OK {
        return Err(KernelError::from_status(status));
    }
    offsets
        .windows(2)
        .map(|range| {
            points
                .get(range[0]..range[1])
                .map(<[[f64; 2]]>::to_vec)
                .ok_or(KernelError::Internal(sys::VE_ERR_OUT_OF_BOUNDS))
        })
        .collect()
}

pub(crate) fn offset(
    profile: &ProfileHandle,
    delta: f64,
    round: bool,
) -> Result<ProfileHandle, KernelError> {
    let mut status = sys::VE_OK;
    // SAFETY: live borrowed handle and valid output pointer; scalar input is checked in C++.
    let ptr = unsafe {
        sys::ve_profile_offset(profile.0.as_ptr(), delta, i32::from(round), &raw mut status)
    };
    ProfileHandle::from_raw(ptr, status)
}

pub(crate) fn difference(
    a: &ProfileHandle,
    b: &ProfileHandle,
) -> Result<ProfileHandle, KernelError> {
    let mut status = sys::VE_OK;
    // SAFETY: both immutable handles outlive the call; status is valid.
    let ptr = unsafe { sys::ve_profile_difference(a.0.as_ptr(), b.0.as_ptr(), &raw mut status) };
    ProfileHandle::from_raw(ptr, status)
}

pub(crate) fn extrude(
    profile: &ProfileHandle,
    height: f64,
    divisions: i32,
    twist: f64,
    top: [f64; 2],
) -> Result<Handle, KernelError> {
    let mut status = sys::VE_OK;
    // SAFETY: live borrowed profile and valid output pointer; C++ checks all scalars.
    let ptr = unsafe {
        sys::ve_profile_extrude(
            profile.0.as_ptr(),
            height,
            divisions,
            twist,
            top[0],
            top[1],
            &raw mut status,
        )
    };
    Handle::from_raw(ptr, status)
}

pub(crate) fn revolve(
    profile: &ProfileHandle,
    segments: i32,
    angle: f64,
) -> Result<Handle, KernelError> {
    let mut status = sys::VE_OK;
    // SAFETY: live borrowed profile and valid output pointer; C++ checks all scalars.
    let ptr =
        unsafe { sys::ve_profile_revolve(profile.0.as_ptr(), segments, angle, &raw mut status) };
    Handle::from_raw(ptr, status)
}
