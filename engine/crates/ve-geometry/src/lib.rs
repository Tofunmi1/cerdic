//! Typed geometry values: attributes with domains, polygon meshes, and adapters to the kernel.
//!
//! Engine-agnostic: no Bevy, no platform code. A render mesh is a derived triangle buffer; the
//! values here are what operators read and write. Representations are introduced together with the
//! operation that needs them (see `docs/plans/track2-modeling.md`).

mod adapt;
mod attr;
mod error;
mod mesh;

pub use adapt::{LossPolicy, LossReport, polymesh_to_solid, solid_to_polymesh};
pub use attr::{AttrData, Attribute, Domain, Propagation};
pub use error::GeometryError;
pub use mesh::{MAX_ATTRIBUTE_NAME, PolyMesh};
