//! Plant meshing, leaf cards, LOD chains and impostor data (ADR 0056, 0058): turns a `flora` skeleton into the vertex arrays a renderer
//! draws, with bark UVs, baked three-scale wind data, simpler meshes for distance and billboard views for far away.
mod chain;
mod error;
mod impostor;
mod mesh;
pub mod octahedral;
mod tuft;
mod types;

pub use chain::build_lods;
pub use error::LodError;
pub use impostor::{
    OctahedralImpostorFrame, OctahedralImpostorPlan, impostor_spec, octahedral_impostor_plan,
};
pub use mesh::mesh_plant;
pub use tuft::mesh_tuft;
pub use types::{
    ImpostorSpec, ImpostorView, LeafCards, LodChain, MaterialRange, Meshing, PlantMesh, Surface,
    Vertex, WindData,
};
