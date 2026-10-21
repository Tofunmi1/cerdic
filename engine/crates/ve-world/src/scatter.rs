//! Attribute-carrying point sets and locally stable Poisson-disk placement (ADR 0056).
//!
//! A point set holds positions and named channels (scalar or vector, one value per point). Placement is a pure function of the world
//! seed and world coordinates, so a rectangle gives the same points however it is cut into tiles.
mod error;
mod field;
mod place;
mod points;
mod species;

pub use error::ScatterError;
pub use field::Field2;
pub use place::{Poisson, Variation, align_to_surface, cluster, filter, poisson, relax, vary};
pub use points::PointSet;
pub use species::{Rule, Species, place_species};
