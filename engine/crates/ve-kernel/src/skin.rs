//! The Skin modifier (Blender's B-Mesh implementation): a skeleton graph with a pair of radii per joint becomes one closed surface of mostly quads.

use crate::bevel::BeveledMesh;
use crate::{KernelError, ffi};

/// One joint of a skeleton graph.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkinJoint {
    /// Where the joint is.
    pub position: [f32; 3],
    /// Half extents of the cross-section at the joint, across and along its bone frame, in metres (zero or more).
    pub radius: [f32; 2],
    /// The joint the bone frames are oriented from: every connected piece of the graph needs exactly one.
    pub root: bool,
    /// A branch joint whose hull is not subdivided (rarely useful).
    pub loose: bool,
}

/// Parameters of [`skin_graph`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkinOptions {
    /// Relaxes the surface around branches, 0 (none) to 1 (full).
    pub branch_smoothing: f32,
    /// Mirror axes that quads are kept from crossing: bit 0 x, bit 1 y, bit 2 z.
    pub symmetry_axes: u32,
    /// Add joints along long bones so the cross-section rings stay close together (Blender's behaviour).
    pub subdivide: bool,
}

impl Default for SkinOptions {
    fn default() -> Self {
        Self { branch_smoothing: 0.0, symmetry_axes: 0, subdivide: true }
    }
}

/// Skins the graph. The result's `vertex_origin` is the joint (index into `joints`) each vertex was made from.
///
/// The surface stops at an end joint: it does not extend past it. Several thin bones leaving ONE joint collide in the junction
/// (fingers must hang off a chain of knuckles instead).
///
/// # Errors
/// [`KernelError::InvalidArgument`] for a bad radius, a bone joining a joint to itself, or a connected piece with no root;
/// [`KernelError::OutOfBounds`] for a bone to a missing joint; [`KernelError::NonFinite`] for a NaN or infinite position;
/// [`KernelError::TooLarge`] past 4,096 joints or 200,000 output vertices.
pub fn skin_graph(joints: &[SkinJoint], bones: &[[u32; 2]], options: &SkinOptions) -> Result<BeveledMesh, KernelError> {
    ffi::polymesh::skin(joints, bones, options)
}
