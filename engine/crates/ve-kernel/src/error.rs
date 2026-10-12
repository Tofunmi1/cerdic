//! Typed kernel errors.

use ve_kernel_sys::{
    VE_ERR_NON_FINITE, VE_ERR_NOT_MANIFOLD, VE_ERR_OUT_OF_BOUNDS, VE_ERR_TOO_LARGE, VeStatus,
};

/// Why a kernel operation failed. Messages are written to be useful to an LLM repair loop.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum KernelError {
    /// Cooperative evaluation was cancelled; retry with fresh objects and context.
    #[error("evaluation cancelled")]
    Cancelled,
    /// A parameter was invalid (non-finite, non-positive, out of range, or inconsistent).
    #[error("invalid argument: {0}")]
    InvalidArgument(&'static str),
    /// The input mesh, or the result of an operation, is not a closed manifold.
    #[error("the mesh is not a closed manifold (every edge must join exactly two faces)")]
    NotManifold,
    /// A vertex coordinate was NaN or infinite.
    #[error("a vertex coordinate is NaN or infinite")]
    NonFinite,
    /// A triangle referred to a vertex index that does not exist.
    #[error("a triangle index refers to a vertex that does not exist")]
    OutOfBounds,
    /// The result is too large to represent.
    #[error("the result is too large to represent")]
    TooLarge,
    /// The kernel reported an unexpected failure.
    #[error("internal kernel failure (status {0})")]
    Internal(i32),
}

impl KernelError {
    /// Maps a kernel status code to an error. Codes that mean "bad argument" become
    /// [`KernelError::InvalidArgument`] with a generic description.
    pub(crate) fn from_status(code: VeStatus) -> Self {
        match code {
            ve_kernel_sys::VE_ERR_CANCELLED => Self::Cancelled,
            VE_ERR_NOT_MANIFOLD => Self::NotManifold,
            VE_ERR_NON_FINITE => Self::NonFinite,
            VE_ERR_OUT_OF_BOUNDS => Self::OutOfBounds,
            VE_ERR_TOO_LARGE => Self::TooLarge,
            1 => Self::InvalidArgument("rejected by the kernel"),
            other => Self::Internal(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_codes() {
        assert_eq!(
            KernelError::from_status(VE_ERR_NOT_MANIFOLD),
            KernelError::NotManifold
        );
        assert_eq!(
            KernelError::from_status(VE_ERR_NON_FINITE),
            KernelError::NonFinite
        );
        assert_eq!(
            KernelError::from_status(VE_ERR_OUT_OF_BOUNDS),
            KernelError::OutOfBounds
        );
        assert_eq!(
            KernelError::from_status(VE_ERR_TOO_LARGE),
            KernelError::TooLarge
        );
        assert_eq!(KernelError::from_status(99), KernelError::Internal(99));
    }
}
