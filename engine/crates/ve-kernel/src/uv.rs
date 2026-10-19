//! UV unwrapping with Blender's parametrizer (LSCM / ABF++) and island packer.

use crate::{KernelError, PolygonMesh};
use ve_kernel_sys as sys;

/// How an island is flattened.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UvMethod {
    /// Least-squares conformal maps: fast, angle preserving.
    Conformal,
    /// Angle-based flattening (ABF++): less angle distortion on curved islands, slower.
    AngleBased,
}

/// Parameters of [`unwrap_uvs`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnwrapOptions {
    /// Flattening method.
    pub method: UvMethod,
    /// Pack the islands into the unit square. Without it they overlap near the origin.
    pub pack: bool,
    /// Let packing rotate islands to fit tighter.
    pub rotate: bool,
    /// Space between packed islands, as a fraction of the unit square (0 to 0.5).
    pub margin: f32,
}

/// The result of an unwrap.
#[derive(Clone, Debug, PartialEq)]
pub struct UvLayout {
    /// One `(u, v)` per mesh corner, in the mesh's corner order.
    pub uvs: Vec<[f32; 2]>,
    /// Islands that could not be flattened; their corners keep `(0, 0)`.
    pub failed_islands: u32,
}

/// Packing options for [`pack_uvs`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UvPackOptions {
    /// Allow islands to rotate.
    pub rotate: bool,
    /// Keep the original island scale where possible.
    pub keep_density: bool,
    /// Use axis aligned bounds or convex hulls during packing.
    pub shape: UvPackShape,
    /// Fractional spacing between islands.
    pub margin: f32,
}

/// Island packing shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UvPackShape {
    /// Use axis-aligned bounds.
    Aabb,
    /// Use the convex hull of each island.
    Convex,
}

/// Packs an existing corner UV map into the unit square. Discontinuities define island borders.
pub fn pack_uvs(
    mesh: &PolygonMesh<'_>,
    input_uvs: &[[f32; 2]],
    options: &UvPackOptions,
) -> Result<Vec<[f32; 2]>, KernelError> {
    if input_uvs.len() != mesh.corners.len() {
        return Err(KernelError::InvalidArgument(
            "one UV value is required per mesh corner",
        ));
    }
    let flat_positions = mesh.positions.as_flattened();
    let flat_uvs = input_uvs.as_flattened();
    let input = sys::VeUvPackInput {
        struct_size: core::mem::size_of::<sys::VeUvPackInput>(),
        mesh: sys::VePolymeshInput {
            struct_size: core::mem::size_of::<sys::VePolymeshInput>(),
            positions: flat_positions.as_ptr(),
            vertex_count: mesh.positions.len(),
            face_sizes: mesh.face_sizes.as_ptr(),
            face_count: mesh.face_sizes.len(),
            corners: mesh.corners.as_ptr(),
            corner_count: mesh.corners.len(),
        },
        uvs: flat_uvs.as_ptr(),
        rotate: i32::from(options.rotate),
        keep_density: i32::from(options.keep_density),
        shape: match options.shape {
            UvPackShape::Aabb => 0,
            UvPackShape::Convex => 1,
        },
        margin: options.margin,
    };
    let mut output = input_uvs.to_vec();
    // SAFETY: input slices and output storage remain live for the synchronous C call; capacities
    // match the mesh's corner count and no buffer is retained.
    let status = unsafe {
        sys::ve_uv_pack(
            &raw const input,
            output.as_mut_ptr().cast::<f32>(),
            2 * output.len(),
        )
    };
    if status == sys::VE_OK {
        Ok(output)
    } else {
        Err(KernelError::from_status(status))
    }
}

/// Cuts the mesh along `seams` (vertex pairs, each an edge of the mesh) and flattens every island.
///
/// # Errors
/// [`KernelError::InvalidArgument`] for a bad option, a malformed face, or a seam that is not an edge;
/// [`KernelError::OutOfBounds`] for an index past the vertices; [`KernelError::NonFinite`] for a NaN or
/// infinite coordinate; [`KernelError::TooLarge`] past 1,000,000 vertices, faces or 4,000,000 corners.
pub fn unwrap_uvs(
    mesh: &PolygonMesh<'_>,
    seams: &[[u32; 2]],
    options: &UnwrapOptions,
) -> Result<UvLayout, KernelError> {
    let flat_positions = mesh.positions.as_flattened();
    let flat_seams = seams.as_flattened();
    let input = sys::VeUvInput {
        struct_size: core::mem::size_of::<sys::VeUvInput>(),
        mesh: sys::VePolymeshInput {
            struct_size: core::mem::size_of::<sys::VePolymeshInput>(),
            positions: flat_positions.as_ptr(),
            vertex_count: mesh.positions.len(),
            face_sizes: mesh.face_sizes.as_ptr(),
            face_count: mesh.face_sizes.len(),
            corners: mesh.corners.as_ptr(),
            corner_count: mesh.corners.len(),
        },
        seams: flat_seams.as_ptr(),
        seam_count: seams.len(),
        method: match options.method {
            UvMethod::Conformal => 0,
            UvMethod::AngleBased => 1,
        },
        pack: i32::from(options.pack),
        rotate: i32::from(options.rotate),
        margin: options.margin,
    };
    let mut uvs = vec![[0.0_f32; 2]; mesh.corners.len()];
    let mut failed = 0_u32;
    // SAFETY: `input` points at slices that outlive the call (positions hold `3 * vertex_count` floats,
    // `face_sizes` and `corners` the counts passed with them, `seams` two indices per seam); the C side
    // validates every index. `uvs` holds exactly the `2 * corner_count` floats whose capacity is passed,
    // and `failed` is a valid out-pointer. Nothing is retained after the call.
    let status = unsafe {
        sys::ve_uv_unwrap(
            &raw const input,
            uvs.as_mut_ptr().cast::<f32>(),
            2 * uvs.len(),
            &raw mut failed,
        )
    };
    if status == sys::VE_OK {
        Ok(UvLayout {
            uvs,
            failed_islands: failed,
        })
    } else {
        Err(KernelError::from_status(status))
    }
}
