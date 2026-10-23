//! Pure Rust reader and evaluator for the CC0 human body pack.
mod error;
mod eval;
mod fit;
mod measure;
mod outfit;
mod pack;

pub use error::Error;
pub use eval::{Body, Human, Params, RenderMesh};
pub use fit::{Achieved, FitOptions, FitResult, Target, fit};
pub use measure::{Measure, measure};
pub use outfit::Outfit;
pub use pack::Pack;
