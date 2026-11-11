//! Typed geometry values: attributes with domains, polygon meshes, and adapters to the kernel.
//!
//! Engine-agnostic: no Bevy, no platform code. A render mesh is a derived triangle buffer; the
//! values here are what operators read and write. Representations are introduced together with the
//! operation that needs them.

mod adapt;
mod attr;
mod error;
pub mod field;
mod mesh;
mod poly;
mod subdivide;
mod topology;
mod triangulate;
mod validate;

pub use triangulate::TriangulationMode;

pub use adapt::{LossPolicy, LossReport, polymesh_to_solid, solid_to_polymesh};
pub use attr::{AttrData, Attribute, Domain, Propagation};
pub use error::{Element, GeometryError};
pub use field::{DistanceGuarantee, Field};
pub use mesh::{MAX_ATTRIBUTE_NAME, PolyMesh};
pub use poly::{
    FlipFacesResult, InsetEach, MAX_BEVEL_SEGMENTS, MAX_GRID_FACES, MAX_INDIVIDUAL_FACES,
    MAX_MERGES, MAX_NETWORK_BEVEL_SEGMENTS, NetworkBevel, NetworkBevelOffset, WeldResult,
    auto_seams, bevel_edges, bevel_network, blender_edit, bridge_loops, delete_faces,
    dissolve_edge, dissolve_faces, dissolve_vertex, extrude_each, extrude_faces, flip_component,
    flip_faces, grid, inset_each, inset_face, merge_coplanar, merge_coplanar_keeping_holes,
    pack_uv, unwrap_uv, weld,
};
pub use poly::{UvMethod, UvQuality, UvUnwrap, uv_quality};
pub use subdivide::{Subdivided, SubdivisionOptions, subdivide};
pub use topology::{EdgeInfo, EdgeKind, Topology};
pub use validate::{ValidationReport, validate};
pub use ve_kernel::{MeshOp, PokeCenter};
