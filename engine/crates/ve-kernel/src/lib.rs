//! Safe Rust API over the C++ geometry kernel.
//!
//! A [`Solid`] is an immutable, closed, manifold volume. Operations return new solids and never
//! mutate their inputs. All fallible operations return a typed [`KernelError`]; nothing panics.
//! The only `unsafe` code lives in the private `ffi` module, where every block states its invariant.
#![expect(
    unsafe_code,
    reason = "the audited FFI layer lives in the private `ffi` module"
)]

mod error;
mod ffi;
mod mesh;
mod solid;

pub use error::KernelError;
pub use mesh::Mesh;
pub use solid::{Bounds, Solid, simplify};
