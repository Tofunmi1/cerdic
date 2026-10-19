//! Tangent-space generation for normal-mapped assets.

use crate::{KernelError, ffi};

/// One `MikkTSpace` tangent per triangle corner: the tangent direction and the bitangent sign.
///
/// `MikkTSpace` is the basis normal-map bakers and game engines agree on, so a normal map baked here
/// shades the same elsewhere. The bitangent is `cross(normal, tangent) * sign`. The result has one entry
/// per index, in index order, so corners of a vertex that sits on a UV seam or a hard edge can differ.
///
/// `normals` and `uvs` are per vertex and must be as long as `positions`; `indices` is a multiple of
/// three. At most 2,000,000 triangles.
///
/// # Errors
/// Mismatched lengths or an index count that is not a multiple of three
/// ([`KernelError::InvalidArgument`]), a vertex index out of range ([`KernelError::OutOfBounds`]), a
/// NaN or infinite value ([`KernelError::NonFinite`]), or too many triangles ([`KernelError::TooLarge`]).
pub fn tangents(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    uvs: &[[f32; 2]],
    indices: &[u32],
) -> Result<Vec<[f32; 4]>, KernelError> {
    if positions.len() != normals.len() || positions.len() != uvs.len() {
        return Err(KernelError::InvalidArgument(
            "positions, normals and uvs must have one entry per vertex",
        ));
    }
    ffi::tangents(positions, normals, uvs, indices)
}
