//! Navigation for generated levels: a walkable grid built from triangles, A* plus string pulling,
//! and a small deterministic crowd.
//!
//! Everything here is pure Rust, so it runs on native and `wasm32`, and every result is
//! deterministic: no hash-map iteration order, no order-dependent float sums. Levels are generated
//! from our own geometry, so the nav mesh is generated from the same triangles the renderer draws;
//! nothing is imported.
//!
//! # Pipeline
//! [`NavMesh::build`] rasterises triangles into a heightfield of cells and keeps the surfaces an
//! agent can stand on (slope, headroom, step), then erodes them by the agent radius and stores the
//! result as a walkable grid. [`NavMesh::find_path`] runs A* over that grid and pulls the route
//! tight with the funnel algorithm. [`Crowd`] steers up to [`MAX_AGENTS`] agents along those paths,
//! and [`behaviour`] holds a few goal pickers (`wander`, `patrol`, `chase`, `flee`).
#![expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "grid dimensions and cell coordinates are small; f32 and usize round-trip exactly here"
)]

mod agent;
mod astar;
mod behaviour;
mod build;
mod config;
mod crowd;
mod error;
mod filter;
mod funnel;
mod grid;
mod links;
mod mesh;
mod overlay;
mod path;
mod raycast;
mod search;
mod smart;
mod space;
mod steering;

pub use agent::{Agent, AgentParams, AgentStatus};
pub use behaviour::{Chase, Flee, Goal, Patrol, Wander, line_of_sight};
pub use config::NavConfig;
pub use crowd::{Crowd, MAX_AGENTS};
pub use error::NavError;
pub use filter::CostFilter;
pub use links::{Link, LinkEnd, LinkId, LinkState, LinkedSpace};
pub use mesh::NavMesh;
pub use overlay::{DynamicSpace, ObstacleId, VolumeId};
pub use path::Path;
pub use search::{NodePath, SearchLimit, find_node_path, find_point_path};
pub use smart::{Claim, ClaimError, ObjectId, SlotDef, SmartObjectDef, SmartObjects};
pub use space::{AreaId, Edge, EdgeKind, NavSpace, NodeId};

pub use build::MAX_CELLS;
