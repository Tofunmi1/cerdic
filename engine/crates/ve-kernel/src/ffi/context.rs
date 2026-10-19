//! Audited lifetime and thread-safety of Manifold execution contexts.
use super::{Handle, ensure_initialized};
use crate::KernelError;
use core::ptr::NonNull;
use ve_kernel_sys as sys;

pub(crate) struct Context(NonNull<sys::VeContext>);
// SAFETY: Manifold documents Cancel/Cancelled as thread-safe; ownership stays unique.
unsafe impl Send for Context {}
// SAFETY: shared methods only access Manifold's synchronized cancellation flag.
unsafe impl Sync for Context {}

impl Context {
    pub(crate) fn as_ptr(&self) -> *const sys::VeContext {
        self.0.as_ptr()
    }
    pub(crate) fn as_mut_ptr(&self) -> *mut sys::VeContext {
        self.0.as_ptr()
    }
    pub(crate) fn new() -> Result<Self, KernelError> {
        ensure_initialized();
        // SAFETY: constructor takes no pointers and transfers unique ownership.
        NonNull::new(unsafe { sys::ve_context_new() })
            .map(Self)
            .ok_or(KernelError::Internal(sys::VE_ERR_INTERNAL))
    }
    pub(crate) fn cancel(&self) {
        // SAFETY: the handle is live and Cancel is thread-safe.
        unsafe { sys::ve_context_cancel(self.0.as_ptr()) };
    }
    pub(crate) fn cancelled(&self) -> bool {
        // SAFETY: the borrowed handle remains live, and the query is thread-safe.
        unsafe { sys::ve_context_cancelled(self.0.as_ptr()) != 0 }
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        // SAFETY: unique ownership, no borrowed evaluations can outlive self.
        unsafe { sys::ve_context_free(self.0.as_ptr()) };
    }
}

pub(crate) fn boolean(
    a: &Handle,
    b: &Handle,
    operation: i32,
    context: &Context,
) -> Result<Handle, KernelError> {
    let mut status = sys::VE_OK;
    // SAFETY: inputs/context remain borrowed throughout this synchronous call; status is valid.
    let ptr = unsafe {
        sys::ve_solid_boolean_context(
            a.as_ptr(),
            b.as_ptr(),
            operation,
            context.0.as_ptr(),
            &raw mut status,
        )
    };
    Handle::from_raw(ptr, status)
}

pub(crate) fn union_all(solids: &[&Handle], context: &Context) -> Result<Handle, KernelError> {
    let pointers: Vec<_> = solids.iter().map(|solid| solid.as_ptr()).collect();
    let mut status = sys::VE_OK;
    // SAFETY: every solid/context stays borrowed; the pointer array is valid for its given count.
    let ptr = unsafe {
        sys::ve_solid_batch_union_context(
            pointers.as_ptr(),
            pointers.len(),
            context.0.as_ptr(),
            &raw mut status,
        )
    };
    Handle::from_raw(ptr, status)
}
