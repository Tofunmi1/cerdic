//! Rivers, roads, shores and climate on a height grid (ADR 0056). All geometry is in world metres; a river's bed follows the local ground
//! (so tiles of one world agree), while roads follow a grade-limited profile along their whole path.
mod climate;
mod error;
mod geometry;
mod ground;
mod lake;
mod path;
mod road;
mod shore;
mod stream;

pub use climate::{Climate, moisture, temperature};
pub use error::RiverError;
pub use ground::{Ground, GroundRef};
pub use lake::{LakeProfile, fill_basin};
pub use path::Path;
pub use road::{RoadOut, RoadProfile, carve_road};
pub use shore::{ShoreOut, shore_masks};
pub use stream::{RiverOut, RiverProfile, carve_river, carve_river_descending};
