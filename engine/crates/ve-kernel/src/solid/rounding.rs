//! Whole-solid edge treatment: the bounded Minkowski round and chamfer.
//!
//! Both operations erode then dilate the solid with a convex shape, so they only affect convex edges
//! and vertices and leave concave edges sharp. `round` uses a sphere, `chamfer` an octahedron.

use crate::KernelError;
use crate::ffi;

use super::{Solid, require_positive};

impl Solid {
    /// Rounds edges by a convex Minkowski erosion and dilation with the given radius.
    ///
    /// The source is capped at 20,000 triangles and the result at 250,000 triangles; 6..=24
    /// sphere segments control the rounded profile. Radius must be less than half the thinnest
    /// bounding dimension. The Manifold kernel handles the erosion, so thin or deeply concave
    /// features can still refuse if the eroded core becomes empty.
    ///
    /// # Errors
    /// Invalid radius/segments, a too-small core, or an input/result over the operation caps.
    pub fn round(&self, radius: f64, segments: u32) -> Result<Self, KernelError> {
        require_positive(
            [radius, 1.0, 1.0],
            "round radius must be finite and positive",
        )?;
        if !(6..=24).contains(&segments) {
            return Err(KernelError::InvalidArgument("round segments must be 6..24"));
        }
        if self.triangle_count() > 20_000 {
            return Err(KernelError::TooLarge);
        }
        let size = self.bounds()?.size();
        if size
            .iter()
            .any(|dimension| 2.0 * radius >= dimension * (1.0 - 1.0e-6))
        {
            return Err(KernelError::InvalidArgument(
                "round radius exceeds the solid core",
            ));
        }
        let segments = i32::try_from(segments)
            .map_err(|_| KernelError::InvalidArgument("round segments must be 6..24"))?;
        ffi::round(&self.0, radius, segments).map(Self)
    }

    /// Breaks convex edges with a flat 45-degree facet by eroding then dilating with an octahedron.
    ///
    /// `width` is the chamfer size in metres. Like [`Solid::round`] this only affects convex edges and
    /// vertices, so concave edges stay sharp: it is a chamfer of a convex solid, not arbitrary CAD
    /// filleting. The source is capped at 20,000 triangles and the result at 250,000; `width` must be
    /// less than half the thinnest bounding dimension or the eroded core becomes empty.
    ///
    /// # Errors
    /// Invalid width, a too-small core, or an input/result over the operation caps.
    pub fn chamfer(&self, width: f64) -> Result<Self, KernelError> {
        require_positive(
            [width, 1.0, 1.0],
            "chamfer width must be finite and positive",
        )?;
        if self.triangle_count() > 20_000 {
            return Err(KernelError::TooLarge);
        }
        let size = self.bounds()?.size();
        if size
            .iter()
            .any(|dimension| 2.0 * width >= dimension * (1.0 - 1.0e-6))
        {
            return Err(KernelError::InvalidArgument(
                "chamfer width exceeds the solid core",
            ));
        }
        ffi::chamfer(&self.0, width).map(Self)
    }
}
