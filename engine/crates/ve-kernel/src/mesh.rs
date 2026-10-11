//! Indexed triangle meshes exchanged with the kernel.

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
