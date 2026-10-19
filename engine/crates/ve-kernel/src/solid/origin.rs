//! Origin tracking: which input each triangle of a boolean or transform result came from.
use super::Solid;
use crate::ffi;
use crate::{KernelError, Mesh};

impl Solid {
    /// Reserves `count` fresh origin ids and returns the first; ids are consecutive and unique for
    /// the life of the process.
    #[must_use]
    pub fn reserve_ids(count: u32) -> u32 {
        ffi::reserve_ids(count)
    }

    /// Builds a solid from a mesh whose triangles are grouped in runs. Run `i` starts at triangle
    /// `first_triangles[i]` (the first at 0, ascending) and carries origin id `ids[i]` (ascending,
    /// from [`Solid::reserve_ids`]). Booleans and rigid transforms keep the ids on surviving faces and
    /// give faces cut by a second operand that operand's id, so [`Solid::runs`] can recover which
    /// input each triangle of the result came from.
    ///
    /// # Errors
    /// As for [`Solid::from_mesh`], plus [`KernelError::InvalidArgument`] for malformed runs.
    pub fn from_mesh_runs(
        mesh: &Mesh,
        first_triangles: &[usize],
        ids: &[u32],
    ) -> Result<Self, KernelError> {
        if mesh.positions.is_empty()
            || mesh.indices.is_empty()
            || !mesh.indices.len().is_multiple_of(3)
        {
            return Err(KernelError::InvalidArgument(
                "mesh needs vertices and a non-empty index list divisible by three",
            ));
        }
        if mesh.positions.iter().flatten().any(|c| !c.is_finite()) {
            return Err(KernelError::NonFinite);
        }
        ffi::from_mesh_runs(&mesh.positions, &mesh.indices, first_triangles, ids).map(Self)
    }

    /// The triangle runs of the mesh [`Solid::to_mesh`] copies, as `(first triangle, origin id)`,
    /// ascending by id. The two calls order triangles identically. A run can be empty when copies
    /// coincide, so a run ends where the next begins (or at the last triangle).
    ///
    /// # Errors
    /// Propagates kernel failures; not expected for a live solid.
    pub fn runs(&self) -> Result<Vec<(usize, u32)>, KernelError> {
        let (first, ids) = ffi::runs(&self.0)?;
        Ok(first.into_iter().zip(ids).collect())
    }
}
