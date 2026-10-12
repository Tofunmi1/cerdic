//! Conversions between polygon meshes and the kernel's triangle meshes and solids.
//!
//! The kernel carries positions and triangle indices only. A conversion either keeps what the
//! target can hold and reports the rest ([`LossPolicy::Report`]) or refuses to lose attributes
//! ([`LossPolicy::Reject`]); it never drops them silently.

use ve_kernel::{Mesh, Solid};

use crate::error::GeometryError;
use crate::mesh::PolyMesh;

/// What to do when the target cannot hold some of the source's data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LossPolicy {
    /// Fail if any attribute would be dropped.
    Reject,
    /// Drop them and say which.
    Report,
}

/// What a conversion did not carry over.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LossReport {
    /// Attributes the target has no place for.
    pub dropped_attributes: Vec<String>,
    /// Faces with more than three corners that were split into triangles (as a fan, which is only
    /// right for planar convex faces).
    pub faces_triangulated: usize,
}

impl PolyMesh {
    /// A polygon mesh of the kernel mesh's triangles, with no attributes.
    ///
    /// # Errors
    /// [`GeometryError::InvalidMesh`] if the kernel mesh is inconsistent.
    pub fn from_kernel_mesh(mesh: &Mesh) -> Result<Self, GeometryError> {
        let triangles = mesh.indices.len() / 3;
        Self::new(
            mesh.positions.clone(),
            vec![3; triangles],
            mesh.indices.get(..triangles * 3).unwrap_or(&[]).to_vec(),
        )
    }

    /// The kernel mesh of this mesh, triangulating larger faces as fans.
    ///
    /// # Errors
    /// [`GeometryError::AttributesWouldBeLost`] under [`LossPolicy::Reject`] when the mesh has
    /// attributes (the kernel mesh cannot hold them).
    pub fn to_kernel_mesh(&self, policy: LossPolicy) -> Result<(Mesh, LossReport), GeometryError> {
        let names: Vec<String> = self.attributes().keys().cloned().collect();
        if policy == LossPolicy::Reject && !names.is_empty() {
            return Err(GeometryError::AttributesWouldBeLost(names));
        }
        let mut indices = Vec::with_capacity(self.corners().len());
        let mut triangulated = 0;
        let mut start = 0_usize;
        for &size in self.face_sizes() {
            let size = size as usize;
            let face = self.corners().get(start..start + size).unwrap_or(&[]);
            if let Some((&first, rest)) = face.split_first() {
                for pair in rest.windows(2) {
                    if let [b, c] = pair {
                        indices.extend([first, *b, *c]);
                    }
                }
            }
            triangulated += usize::from(size > 3);
            start += size;
        }
        let mesh = Mesh {
            positions: self.positions().to_vec(),
            indices,
        };
        Ok((
            mesh,
            LossReport {
                dropped_attributes: names,
                faces_triangulated: triangulated,
            },
        ))
    }
}

/// The triangles of a solid as a polygon mesh.
///
/// # Errors
/// [`GeometryError::Kernel`] if the kernel cannot produce the mesh.
pub fn solid_to_polymesh(solid: &Solid) -> Result<PolyMesh, GeometryError> {
    PolyMesh::from_kernel_mesh(&solid.to_mesh()?)
}

/// A solid from a polygon mesh, which must be a closed manifold.
///
/// # Errors
/// An attribute-loss error under [`LossPolicy::Reject`], or [`GeometryError::Kernel`] if the mesh
/// is not a valid closed solid.
pub fn polymesh_to_solid(
    mesh: &PolyMesh,
    policy: LossPolicy,
) -> Result<(Solid, LossReport), GeometryError> {
    let (kernel, report) = mesh.to_kernel_mesh(policy)?;
    Ok((Solid::from_mesh(&kernel)?, report))
}
