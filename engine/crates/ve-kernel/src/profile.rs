//! Owned, bounded planar profiles and Y-up solid factories.
use crate::{KernelError, Solid, ffi::profile as ffi};

/// An immutable, validated even-odd planar region. Empty boolean/offset results are valid.
pub struct Profile(ffi::ProfileHandle);

impl Profile {
    /// One simple closed XY polygon; closing point is implicit.
    ///
    /// # Errors
    /// Non-finite, oversized, degenerate or self-intersecting input.
    pub fn polygon(points: &[[f64; 2]]) -> Result<Self, KernelError> {
        if !(3..=4096).contains(&points.len()) {
            return Err(KernelError::InvalidArgument("polygon needs 3..4096 points"));
        }
        Self::from_loops(&[points.to_vec()])
    }

    /// Constructs up to 64 even-odd loops, 4096 points each and 16384 total. No loops means empty.
    ///
    /// # Errors
    /// Invalid loop geometry, coordinates or sizes.
    pub fn from_loops(loops: &[Vec<[f64; 2]>]) -> Result<Self, KernelError> {
        if loops.len() > 64 || loops.iter().any(|p| !(3..=4096).contains(&p.len())) {
            return Err(KernelError::InvalidArgument("profile loop budget"));
        }
        let count = loops.iter().map(Vec::len).sum::<usize>();
        if count > 16384 {
            return Err(KernelError::InvalidArgument("profile point budget"));
        }
        let mut points = Vec::with_capacity(count);
        let mut offsets = vec![0];
        for loop_points in loops {
            if loop_points
                .iter()
                .flatten()
                .any(|v| !v.is_finite() || v.abs() > crate::MAX_EXTENT)
            {
                return Err(KernelError::InvalidArgument("profile coordinates"));
            }
            points.extend_from_slice(loop_points);
            offsets.push(points.len());
        }
        ffi::new(&points, &offsets).map(Self)
    }

    /// Exports canonical f64 loops, without leaking an opaque handle into authored data or caches.
    ///
    /// # Errors
    /// Invalid backend counts or failed copy.
    pub fn loops(&self) -> Result<Vec<Vec<[f64; 2]>>, KernelError> {
        ffi::loops(&self.0)
    }

    /// Filled area in square metres.
    #[must_use]
    pub fn area(&self) -> f64 {
        ffi::area(&self.0)
    }

    /// Whether the filled region is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.area() <= 0.0
    }

    /// Offset in metres with mitred corners (miter limit 2); erosion may validly return empty.
    ///
    /// # Errors
    /// Invalid distance or oversized output.
    pub fn offset(&self, delta: f64) -> Result<Self, KernelError> {
        ffi::offset(&self.0, delta, false).map(Self)
    }

    /// Offset in metres with round corners (64 segments per full circle), so growing a convex
    /// outline rounds its corners and shrinking one rounds the hollows. Erosion may return empty.
    ///
    /// # Errors
    /// Invalid distance or oversized output.
    pub fn offset_round(&self, delta: f64) -> Result<Self, KernelError> {
        ffi::offset(&self.0, delta, true).map(Self)
    }

    /// Rounds convex corners while preserving the outer bounds by eroding with mitered corners,
    /// then growing with round joins.
    ///
    /// # Errors
    /// Invalid distance, an eroded-away profile, oversized output or kernel failure.
    pub fn fillet(&self, radius: f64) -> Result<Self, KernelError> {
        if !radius.is_finite() || radius <= 0.0 || radius > crate::MAX_EXTENT {
            return Err(KernelError::InvalidArgument(
                "fillet radius must be finite and positive",
            ));
        }
        let core = self.offset(-radius)?;
        if core.is_empty() {
            return Err(KernelError::InvalidArgument(
                "fillet radius removes profile core",
            ));
        }
        core.offset_round(radius)
    }

    /// Removes the other profile without changing either input.
    ///
    /// # Errors
    /// Oversized result or kernel failure.
    pub fn difference(&self, other: &Self) -> Result<Self, KernelError> {
        ffi::difference(&self.0, &other.0).map(Self)
    }

    /// Extrudes along Y. Profile Y becomes -Z; twist is radians; divisions count vertical slabs.
    ///
    /// # Errors
    /// Height must be positive, divisions 1..256, scale positive and all values finite/bounded.
    pub fn extrude(
        &self,
        height: f64,
        divisions: u32,
        twist: f64,
        top_scale: [f64; 2],
    ) -> Result<Solid, KernelError> {
        let divisions =
            i32::try_from(divisions).map_err(|_| KernelError::InvalidArgument("divisions"))?;
        ffi::extrude(&self.0, height, divisions, twist, top_scale).map(Solid::from_handle)
    }

    /// Revolves a nonnegative-radius XY profile about Y. Angle is radians in (0, 2π].
    ///
    /// # Errors
    /// Segments outside 3..512, invalid angle/radius, oversized output or kernel failure.
    pub fn revolve(&self, segments: u32, angle: f64) -> Result<Solid, KernelError> {
        let segments =
            i32::try_from(segments).map_err(|_| KernelError::InvalidArgument("segments"))?;
        ffi::revolve(&self.0, segments, angle).map(Solid::from_handle)
    }
}
