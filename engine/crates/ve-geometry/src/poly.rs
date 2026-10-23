//! Transactional polygon edit operators.

mod attrs;
mod auto_seams;
mod bevel_edges;
mod bevel_network;
mod blender_edit;
mod blender_result;
mod bridge_loops;
mod delete_faces;
mod dissolve_edge;
mod dissolve_faces;
mod dissolve_vertex;
mod extrude_faces;
mod flip;
mod individual;
mod inset_face;
mod merge_coplanar;
mod overlap;
mod pack_uv;
mod unwrap_uv;
mod uv_quality;
mod weld;

pub use auto_seams::auto_seams;
pub use bevel_edges::{MAX_BEVEL_SEGMENTS, bevel_edges};
pub use bevel_network::{
    MAX_NETWORK_BEVEL_SEGMENTS, NetworkBevel, NetworkBevelOffset, bevel_network,
};
pub use blender_edit::blender_edit;
pub use bridge_loops::bridge_loops;
pub use delete_faces::delete_faces;
pub use dissolve_edge::dissolve_edge;
pub use dissolve_faces::dissolve_faces;
pub use dissolve_vertex::dissolve_vertex;
pub use extrude_faces::extrude_faces;
pub use flip::{FlipFacesResult, flip_component, flip_faces};
pub use individual::{
    InsetEach, MAX_GRID_FACES, MAX_INDIVIDUAL_FACES, extrude_each, grid, inset_each,
};
pub use inset_face::inset_face;
pub use merge_coplanar::{MAX_MERGES, merge_coplanar, merge_coplanar_keeping_holes};
pub use pack_uv::pack_uv;
pub use unwrap_uv::{UvMethod, UvUnwrap, unwrap_uv};
pub use uv_quality::{UvQuality, uv_quality};
pub use weld::{WeldResult, weld};
