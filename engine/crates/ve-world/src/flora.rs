//! Procedural plant skeletons (ADR 0056): parametric trees, space-colonization crowns and grass tufts, as nodes with a position, radius,
//! parent, branch depth and wind data. Meshing, LODs and impostors are the `lod` module's job.
mod colonize;
mod error;
mod math;
mod preset;
mod random;
mod simplify;
mod skeleton;
mod tree;
mod tuft;
mod wind;

pub use colonize::{Colonize, Crown, colonize};
pub use error::FloraError;
pub use preset::{NAMES, Preset, preset};
pub use simplify::simplify;
pub use skeleton::{Leaf, Skeleton};
pub use tree::{Level, Tree, grow_tree};
pub use tuft::{Tuft, grow_tuft};
