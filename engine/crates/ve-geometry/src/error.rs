//! Errors of geometry construction and conversion.

use ve_kernel::KernelError;

use crate::attr::Domain;

/// Why a geometry operation failed.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum GeometryError {
    /// The mesh data is inconsistent.
    #[error("invalid mesh: {0}")]
    InvalidMesh(&'static str),
    /// The attribute name is empty or too long.
    #[error("bad attribute name {0:?}")]
    BadAttributeName(String),
    /// A mesh has no elements of this kind.
    #[error("meshes have no {0:?} domain")]
    DomainNotOnMesh(Domain),
    /// The number of values does not match the number of elements.
    #[error("attribute {name:?} has {found} values but its domain has {expected} elements")]
    AttributeLength {
        /// The attribute.
        name: String,
        /// Elements in its domain.
        expected: usize,
        /// Values it holds.
        found: usize,
    },
    /// The target representation cannot hold these attributes and the caller refused to lose them.
    #[error("converting would lose the attributes {0:?}")]
    AttributesWouldBeLost(Vec<String>),
    /// The kernel rejected the mesh.
    #[error("kernel: {0}")]
    Kernel(#[from] KernelError),
}
