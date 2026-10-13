//! The public, safe solid type.

use ve_kernel_sys::VE_MAX_SEGMENTS;

use crate::ffi::{self, Handle};
use crate::{KernelError, Mesh};

/// An axis-aligned bounding box in metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    /// Smallest corner.
    pub min: [f64; 3],
    /// Largest corner.
    pub max: [f64; 3],
}

impl Bounds {
    /// Edge lengths along x, y and z.
    #[must_use]
    pub fn size(&self) -> [f64; 3] {
        [
            self.max[0] - self.min[0],
            self.max[1] - self.min[1],
            self.max[2] - self.min[2],
        ]
    }
}

/// An immutable closed manifold volume. Operations return new solids; inputs are never changed.
pub struct Solid(Handle);

impl std::fmt::Debug for Solid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Solid")
            .field("triangles", &self.triangle_count())
            .field("volume", &self.volume())
            .finish()
    }
}

/// The largest dimension, offset or coordinate accepted, in metres. Meshes are rendered in `f32`,
/// whose precision at a thousand kilometres is already coarser than a metre, and a finite `f64`
/// beyond `f32`'s range would turn into infinities on the way out of the kernel. Rejecting such
/// values at the boundary keeps every result renderable.
pub const MAX_EXTENT: f64 = 1.0e6;

fn require_positive(values: [f64; 3], what: &'static str) -> Result<(), KernelError> {
    if values
        .iter()
        .all(|v| v.is_finite() && *v > 0.0 && *v <= MAX_EXTENT)
    {
        Ok(())
    } else {
        Err(KernelError::InvalidArgument(what))
    }
}

fn require_finite(values: [f64; 3], what: &'static str) -> Result<(), KernelError> {
    if values
        .iter()
        .all(|v| v.is_finite() && v.abs() <= MAX_EXTENT)
    {
        Ok(())
    } else {
        Err(KernelError::InvalidArgument(what))
    }
}

fn segments_arg(segments: u32) -> Result<i32, KernelError> {
    if (3..=VE_MAX_SEGMENTS).contains(&segments) {
        i32::try_from(segments).map_err(|_| KernelError::InvalidArgument("segments"))
    } else {
        Err(KernelError::InvalidArgument(
            "segments must be between 3 and 4096",
        ))
    }
}

impl Solid {
    /// Applies a proper rigid row-major 3x4 matrix (translation at indices 3, 7 and 11).
    ///
    /// # Errors
    /// Non-finite/non-rigid matrices, reflection, scale or kernel failure.
    pub fn transform(&self, matrix: [f64; 12]) -> Result<Self, KernelError> {
        ffi::rigid::transform(&self.0, &matrix).map(Self)
    }
    pub(crate) fn from_handle(handle: Handle) -> Self {
        Self(handle)
    }
    /// Unions 2..64 solids with cancellation between boolean-tree operations.
    ///
    /// # Errors
    /// Invalid count, kernel failure or permanent cancellation.
    pub fn union_all_with_context(
        solids: &[&Self],
        context: &crate::ExecutionContext,
    ) -> Result<Self, KernelError> {
        if !(2..=64).contains(&solids.len()) {
            return Err(KernelError::InvalidArgument(
                "batch boolean count must be 2..64",
            ));
        }
        let handles: Vec<_> = solids.iter().map(|solid| &solid.0).collect();
        ffi::context::union_all(&handles, &context.0).map(Self)
    }
    /// Union with cooperative cancellation while evaluating its boolean tree.
    ///
    /// # Errors
    /// Kernel errors or [`KernelError::Cancelled`].
    pub fn union_with_context(
        &self,
        other: &Self,
        context: &crate::ExecutionContext,
    ) -> Result<Self, KernelError> {
        ffi::context::boolean(&self.0, &other.0, 0, &context.0).map(Self)
    }
    /// Difference with cooperative cancellation during boolean evaluation.
    ///
    /// # Errors
    /// Kernel errors or [`KernelError::Cancelled`].
    pub fn difference_with_context(
        &self,
        other: &Self,
        context: &crate::ExecutionContext,
    ) -> Result<Self, KernelError> {
        ffi::context::boolean(&self.0, &other.0, 1, &context.0).map(Self)
    }
    /// Intersection with cooperative cancellation during boolean evaluation.
    ///
    /// # Errors
    /// Kernel errors or [`KernelError::Cancelled`].
    pub fn intersect_with_context(
        &self,
        other: &Self,
        context: &crate::ExecutionContext,
    ) -> Result<Self, KernelError> {
        ffi::context::boolean(&self.0, &other.0, 2, &context.0).map(Self)
    }
    /// A box with the given edge lengths, optionally centred on the origin.
    ///
    /// # Errors
    /// [`KernelError::InvalidArgument`] if any edge length is not finite and positive.
    pub fn cube(size: [f64; 3], center: bool) -> Result<Self, KernelError> {
        require_positive(size, "cube size must be finite and positive")?;
        ffi::cube(size, center).map(Self)
    }

    /// A sphere of `radius` metres built from `segments` circular segments.
    ///
    /// # Errors
    /// [`KernelError::InvalidArgument`] for a non-positive radius or `segments` outside 3..=4096.
    pub fn sphere(radius: f64, segments: u32) -> Result<Self, KernelError> {
        require_positive(
            [radius, 1.0, 1.0],
            "sphere radius must be finite and positive",
        )?;
        ffi::sphere(radius, segments_arg(segments)?).map(Self)
    }

    /// A cylinder (or cone frustum) along Y with the given end radii.
    ///
    /// # Errors
    /// [`KernelError::InvalidArgument`] for a non-positive height, a negative or both-zero radius, or
    /// `segments` outside 3..=4096.
    pub fn cylinder(
        height: f64,
        radius_low: f64,
        radius_high: f64,
        segments: u32,
        center: bool,
    ) -> Result<Self, KernelError> {
        require_positive(
            [height, 1.0, 1.0],
            "cylinder height must be finite and positive",
        )?;
        let radii_ok = radius_low.is_finite()
            && radius_high.is_finite()
            && (0.0..=MAX_EXTENT).contains(&radius_low)
            && (0.0..=MAX_EXTENT).contains(&radius_high)
            && (radius_low > 0.0 || radius_high > 0.0);
        if !radii_ok {
            return Err(KernelError::InvalidArgument(
                "cylinder radii must be finite, non-negative and not both zero",
            ));
        }
        ffi::cylinder(
            height,
            radius_low,
            radius_high,
            segments_arg(segments)?,
            center,
        )
        .map(Self)
    }

    /// Builds a solid from an indexed triangle mesh. Coincident vertices are welded.
    ///
    /// # Errors
    /// [`KernelError::InvalidArgument`] for an empty mesh or an index count not divisible by three,
    /// [`KernelError::NonFinite`] for NaN/infinite coordinates, [`KernelError::OutOfBounds`] for bad
    /// indices, and [`KernelError::NotManifold`] if the mesh is not a closed manifold.
    pub fn from_mesh(mesh: &Mesh) -> Result<Self, KernelError> {
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
        let count = mesh.positions.len();
        if mesh
            .indices
            .iter()
            .any(|&i| usize::try_from(i).map_or(true, |i| i >= count))
        {
            return Err(KernelError::OutOfBounds);
        }
        ffi::from_mesh(&mesh.positions, &mesh.indices).map(Self)
    }

    /// Boolean union with `other`.
    ///
    /// # Errors
    /// Propagates kernel failures such as [`KernelError::TooLarge`].
    pub fn union(&self, other: &Self) -> Result<Self, KernelError> {
        ffi::union(&self.0, &other.0).map(Self)
    }

    /// Boolean difference: `self` minus `other`.
    ///
    /// # Errors
    /// Propagates kernel failures such as [`KernelError::TooLarge`].
    pub fn difference(&self, other: &Self) -> Result<Self, KernelError> {
        ffi::difference(&self.0, &other.0).map(Self)
    }

    /// Boolean intersection with `other`. The result may be empty.
    ///
    /// # Errors
    /// Propagates kernel failures such as [`KernelError::TooLarge`].
    pub fn intersect(&self, other: &Self) -> Result<Self, KernelError> {
        ffi::intersect(&self.0, &other.0).map(Self)
    }

    /// Moves the solid by `by` metres.
    ///
    /// # Errors
    /// [`KernelError::InvalidArgument`] if any component is not finite.
    pub fn translate(&self, by: [f64; 3]) -> Result<Self, KernelError> {
        require_finite(by, "translation must be finite")?;
        ffi::translate(&self.0, by).map(Self)
    }

    /// Rotates by Euler angles in degrees (applied about x, then y, then z).
    ///
    /// # Errors
    /// [`KernelError::InvalidArgument`] if any angle is not finite.
    pub fn rotate(&self, degrees: [f64; 3]) -> Result<Self, KernelError> {
        require_finite(degrees, "rotation must be finite")?;
        ffi::rotate(&self.0, degrees).map(Self)
    }

    /// Scales per axis.
    ///
    /// # Errors
    /// [`KernelError::InvalidArgument`] if any factor is not finite or is zero.
    pub fn scale(&self, by: [f64; 3]) -> Result<Self, KernelError> {
        require_finite(by, "scale must be finite")?;
        if by.contains(&0.0) {
            return Err(KernelError::InvalidArgument(
                "scale factors must be non-zero",
            ));
        }
        ffi::scale(&self.0, by).map(Self)
    }

    /// Volume in cubic metres.
    #[must_use]
    pub fn volume(&self) -> f64 {
        ffi::volume(&self.0)
    }

    /// Surface area in square metres.
    #[must_use]
    pub fn surface_area(&self) -> f64 {
        ffi::surface_area(&self.0)
    }

    /// Number of vertices in the render mesh.
    #[must_use]
    pub fn vertex_count(&self) -> usize {
        ffi::num_verts(&self.0)
    }

    /// Number of triangles in the render mesh.
    #[must_use]
    pub fn triangle_count(&self) -> usize {
        ffi::num_tris(&self.0)
    }

    /// Number of connected pieces, including sealed internal voids.
    #[must_use]
    pub fn component_count(&self) -> usize {
        ffi::num_components(&self.0)
    }

    /// Whether the solid has no geometry (for example an intersection of disjoint solids).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        ffi::is_empty(&self.0)
    }

    /// The axis-aligned bounding box.
    ///
    /// # Errors
    /// Propagates kernel failures; not expected for a live solid.
    pub fn bounds(&self) -> Result<Bounds, KernelError> {
        let b = ffi::bounds(&self.0)?;
        Ok(Bounds {
            min: [b[0], b[1], b[2]],
            max: [b[3], b[4], b[5]],
        })
    }

    /// Copies the render mesh out of the kernel.
    ///
    /// # Errors
    /// Propagates kernel failures; not expected for a live solid.
    pub fn to_mesh(&self) -> Result<Mesh, KernelError> {
        let mut positions = vec![[0.0_f32; 3]; self.vertex_count()];
        let mut indices = vec![0_u32; self.triangle_count() * 3];
        ffi::copy_mesh(&self.0, &mut positions, &mut indices)?;
        Ok(Mesh { positions, indices })
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
