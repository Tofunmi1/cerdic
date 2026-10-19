//! Errors of geometry construction and conversion.

use ve_kernel::KernelError;

use crate::attr::Domain;

/// Mesh element associated with a typed polygon-operator refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Element {
    /// Transient input face ordinal; graph diagnostics resolve it to a semantic tag.
    Face(usize),
    /// Sorted undirected edge vertex ids.
    Edge(u32, u32),
    /// Vertex id in the input mesh.
    Vertex(u32),
}

/// Why a geometry operation failed.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum GeometryError {
    /// The mesh data is inconsistent.
    #[error("invalid mesh: {0}")]
    InvalidMesh(&'static str),
    /// A bounded geometry walk was cancelled by its execution context.
    #[error("geometry operation cancelled")]
    Cancelled,
    /// A polygon cannot be converted to triangles without changing its authored surface.
    #[error("polygon triangulation is unsupported: {0}")]
    TriangulationUnsupported(&'static str),
    /// An edit is outside the published polygon-operator contract.
    #[error("polygon operator refused the edit: {code} {element:?}")]
    PolyRefusal {
        /// Stable operator diagnostic code.
        code: &'static str,
        /// Input element that caused the refusal, when applicable.
        element: Option<Element>,
    },
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
    /// A spatial-field constructor received a non-finite or out-of-range parameter.
    #[error("invalid spatial field: {0}")]
    FieldParameter(&'static str),
    /// A compiled field expression exceeded the instruction limit.
    #[error("field tape exceeds its 4096-instruction limit ({0} instructions)")]
    FieldTapeLimit(usize),
    /// Field meshing rejected a clipped, empty or over-budget bounded domain.
    #[error(transparent)]
    FieldKernel(#[from] ve_kernel::FieldError),
}
