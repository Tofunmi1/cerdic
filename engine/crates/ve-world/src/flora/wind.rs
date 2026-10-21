//! Shared per-node wind attributes for plant skeletons.

use super::{Skeleton, random::SplitMix64};

/// Appends wind metadata for one node, keeping the phase constant along its branch.
pub(super) fn append(
    skeleton: &mut Skeleton,
    branch: u32,
    depth: u8,
    height_fraction: f32,
    branch_fraction: f32,
    phase: f32,
) {
    let depth_weight = f32::from(depth) * 0.18;
    skeleton
        .wind
        .push((depth_weight + 0.55 * height_fraction).clamp(0.0, 1.0));
    skeleton.phase.push(phase);
    skeleton
        .stiffness
        .push((1.0 - 0.55 * branch_fraction - 0.16 * f32::from(depth)).clamp(0.1, 1.0));
    skeleton.branch.push(branch);
}

/// Creates a stable phase for a branch identifier.
pub(super) fn branch_phase(seed: u32, branch: u32) -> f32 {
    let mut random = SplitMix64::new(seed ^ branch.wrapping_mul(0x9e37_79b9));
    random.unit()
}
