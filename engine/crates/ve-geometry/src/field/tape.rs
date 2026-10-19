//! Public field-tape operations and bounded solid conversion.
mod eval;
mod lower;

use crate::GeometryError;
use crate::field::model::Field;
use ve_kernel::{ExecutionContext, Solid};

pub use eval::evaluate;
pub use lower::{MAX_INSTRUCTIONS, compile};

/// Compiles and meshes a field over explicit bounds.
///
/// # Errors
/// Invalid edge length/bounds, exceeded work caps, clipped or empty domains, cancellation, or a
/// kernel failure.
pub fn mesh(
    field: &Field,
    bounds: [[f64; 3]; 2],
    edge_length: f64,
    context: &ExecutionContext,
) -> Result<Solid, GeometryError> {
    let tape = compile(field)?;
    Solid::from_field(&tape, bounds, edge_length, context).map_err(GeometryError::from)
}
