//! Raw checked profile API; mirrors `ve_kernel.h`.
use crate::{VeSolid, VeStatus};

/// Opaque, exclusively owned 2D cross section.
#[repr(C)]
pub struct VeProfile {
    _private: [u8; 0],
}

// SAFETY: declarations match the linked C++ ABI.
unsafe extern "C" {
    /// Creates even-odd loops from flat points and loop offsets.
    pub fn ve_profile_new(
        points: *const f64,
        point_count: usize,
        offsets: *const usize,
        loop_count: usize,
        status: *mut VeStatus,
    ) -> *mut VeProfile;
    /// Releases an owned profile.
    pub fn ve_profile_free(profile: *mut VeProfile);
    /// Filled area.
    pub fn ve_profile_area(profile: *const VeProfile) -> f64;
    /// Queries bounded point and loop counts.
    pub fn ve_profile_counts(
        profile: *const VeProfile,
        points: *mut usize,
        loops: *mut usize,
    ) -> VeStatus;
    /// Copies XY doubles and offsets into caller-owned buffers.
    pub fn ve_profile_copy(
        profile: *const VeProfile,
        points: *mut f64,
        point_capacity: usize,
        offsets: *mut usize,
        offset_capacity: usize,
    ) -> VeStatus;
    /// Offset with miter (0) or round (1) corners; empty erosion is valid.
    pub fn ve_profile_offset(
        profile: *const VeProfile,
        delta: f64,
        join: i32,
        status: *mut VeStatus,
    ) -> *mut VeProfile;
    /// Difference of borrowed profiles.
    pub fn ve_profile_difference(
        a: *const VeProfile,
        b: *const VeProfile,
        status: *mut VeStatus,
    ) -> *mut VeProfile;
    /// Y-up extrusion, radians, slab count and XY top scale.
    pub fn ve_profile_extrude(
        profile: *const VeProfile,
        height: f64,
        divisions: i32,
        twist: f64,
        top_x: f64,
        top_y: f64,
        status: *mut VeStatus,
    ) -> *mut VeSolid;
    /// Revolve about Y with exactly the requested full-circle segment count.
    pub fn ve_profile_revolve(
        profile: *const VeProfile,
        segments: i32,
        angle: f64,
        status: *mut VeStatus,
    ) -> *mut VeSolid;
}
