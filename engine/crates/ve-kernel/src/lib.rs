//! Safe Rust API over the C++ geometry kernel.
//!
//! A [`Solid`] is an immutable, closed, manifold volume. Operations return new solids and never
//! mutate their inputs. All fallible operations return a typed [`KernelError`]; nothing panics.
//! The only `unsafe` code lives in the private `ffi` module, where every block states its invariant.
#![expect(
    unsafe_code,
    reason = "the audited FFI layer lives in the private `ffi` module"
)]

mod bevel;
mod context;
mod error;
mod ffi;
mod field;
mod mesh;
mod mesh_ops;
mod profile;
mod solid;
mod subdivision;
mod tangents;
mod uv;

pub use bevel::{BevelOffset, BevelOptions, BeveledMesh, PolygonMesh, bevel_edges};
pub use context::ExecutionContext;
pub use error::KernelError;
pub use field::{
    FIELD_MAX_CELLS, FIELD_MAX_INSTRUCTIONS, FIELD_MAX_OPERATIONS, FieldError, FieldInstruction,
    FieldOp, evaluate_tape,
};
pub use mesh::{Mesh, simplify};
pub use mesh_ops::{EditedMesh, MeshOp, MeshSelection, PokeCenter, edit_mesh};
pub use profile::Profile;
pub use solid::{Bounds, MAX_EXTENT, Solid};
pub use subdivision::{
    CornerSharpness, Crease, SubdivisionChannels, SubdivisionMesh, subdivide_catmark,
    subdivide_catmark_with_channels, subdivide_catmark_with_features,
};
pub use tangents::tangents;
pub use uv::{UnwrapOptions, UvLayout, UvMethod, UvPackOptions, UvPackShape, pack_uvs, unwrap_uvs};

/// Blender's own easing curves (`BLI_easing`, "ease out", keyframe defaults), to check the engine's copies against.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlenderEase {
    /// Overshoots the end and returns.
    Back,
    /// Bounces against the end.
    Bounce,
    /// Springs past the end and oscillates down.
    Elastic,
}

/// Evaluates one of Blender's easing curves at `fraction` (0 to 1).
#[must_use]
pub fn blender_ease(curve: BlenderEase, fraction: f32) -> f32 {
    ffi::blender_ease(
        match curve {
            BlenderEase::Back => 0,
            BlenderEase::Bounce => 1,
            BlenderEase::Elastic => 2,
        },
        fraction,
    )
}
