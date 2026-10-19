//! Indexed triangle meshes exchanged with the kernel.

use crate::{KernelError, ffi};

/// An indexed triangle mesh: positions in metres, three indices per triangle.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    /// Vertex positions.
    pub positions: Vec<[f32; 3]>,
    /// Triangle vertex indices, a multiple of three.
    pub indices: Vec<u32>,
}

impl Mesh {
    /// Number of vertices.
    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    /// Number of triangles.
    #[must_use]
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Whether the mesh has no triangles.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }
}

/// Simplifies a mesh to roughly `target_ratio` of its indices (0 < ratio <= 1), within
/// `target_error` relative to the mesh extent. Vertices are kept; only the index list shrinks.
///
/// # Errors
/// [`KernelError::InvalidArgument`] for a ratio outside (0, 1], a negative or non-finite error, or an
/// invalid mesh; [`KernelError::OutOfBounds`] for bad indices.
pub fn simplify(mesh: &Mesh, target_ratio: f64, target_error: f64) -> Result<Mesh, KernelError> {
    if !(target_ratio.is_finite() && target_ratio > 0.0 && target_ratio <= 1.0) {
        return Err(KernelError::InvalidArgument(
            "target ratio must be in (0, 1]",
        ));
    }
    if !(target_error.is_finite() && target_error >= 0.0) {
        return Err(KernelError::InvalidArgument(
            "target error must be finite and non-negative",
        ));
    }
    if mesh.indices.is_empty() || !mesh.indices.len().is_multiple_of(3) {
        return Err(KernelError::InvalidArgument(
            "mesh index count must be a non-zero multiple of three",
        ));
    }
    if mesh.positions.iter().flatten().any(|c| !c.is_finite()) {
        return Err(KernelError::NonFinite);
    }
    let count = mesh.positions.len();
    if mesh
        .indices
        .iter()
        .any(|&i| usize::try_from(i).map_or(true, |i| i >= count))
    {
        return Err(KernelError::OutOfBounds);
    }
    let indices = ffi::simplify(&mesh.positions, &mesh.indices, target_ratio, target_error)?;
    Ok(Mesh {
        positions: mesh.positions.clone(),
        indices,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts() {
        let mesh = Mesh {
            positions: vec![[0.0; 3]; 4],
            indices: vec![0, 1, 2, 0, 2, 3],
        };
        assert_eq!((mesh.vertex_count(), mesh.triangle_count()), (4, 2));
        assert!(!mesh.is_empty());
        assert!(Mesh::default().is_empty());
    }
}
