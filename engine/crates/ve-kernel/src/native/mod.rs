//! Native Rust implementations of the kernel's mesh operators (`edit_mesh`, `bevel_edges`, `skin_graph`,
//! `unwrap_uvs`, `pack_uvs`).
//!
//! Each function here returns `None` when it does not implement the request (a mesh shape or parameter combination it
//! declines); the public API in `lib.rs` then reports [`crate::KernelError::OperatorRefused`]. The module is public
//! (hidden) so tests can call the native code directly.
//!
//! One file per operator family. This is a clean-room implementation: behaviour comes from the doc comments, the
//! contract tests and closed-form geometry.

pub mod basic;
pub mod bevel;
pub mod edit;
pub mod more;
pub mod more_a;
pub mod more_b;
pub mod more_c;
pub mod shape;
pub mod skin;
pub mod subdivide;
pub mod unsubdivide;
pub mod uv;
