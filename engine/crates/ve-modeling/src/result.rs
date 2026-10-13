//! Plain geometry output and explicit quality budgets.
use serde::{Deserialize, Serialize};
use ve_graph::NodeId;

/// Requested tessellation/budget class; part of every cache and job key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    /// Interactive work, at most 200,000 output triangles.
    Preview,
    /// Final output, at most 2,000,000 triangles.
    Final,
}

impl Quality {
    pub(crate) fn triangles(self) -> usize {
        match self {
            Self::Preview => 200_000,
            Self::Final => 2_000_000,
        }
    }
}

/// A solid's surface, as set by a `solid.material` node. Colour is sRGB, emissive is linear.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Material {
    /// Base colour, each 0 to 1.
    pub color: [f32; 3],
    /// 0 dielectric, 1 metal.
    pub metallic: f32,
    /// 0 mirror, 1 matte.
    pub roughness: f32,
    /// Light given off; above 1 glows.
    pub emissive: [f32; 3],
}

/// One named output with its stable authored identity. Mesh data is transferred separately on web.
#[derive(Clone, Debug, Serialize)]
pub struct Part {
    /// Authored source node; never an ECS entity or triangle index.
    pub node: NodeId,
    /// Output name.
    pub name: String,
    /// Render positions in metres, an explicit f32 boundary.
    #[serde(skip)]
    pub positions: Vec<[f32; 3]>,
    /// Triangle indices.
    #[serde(skip)]
    pub indices: Vec<u32>,
    /// Volume of the evaluated solid.
    pub volume: f64,
    /// The surface given by the nearest `solid.material`, if any.
    pub material: Option<Material>,
    /// Column-major positive-determinant render placements; None means an ordinary solid.
    #[serde(skip)]
    pub transforms: Option<Vec<[f32; 16]>>,
    /// Original placement indices, retained when reflected prototypes are separated.
    #[serde(skip)]
    pub instance_indices: Vec<u32>,
}

/// A successful complete evaluation. Failures never yield a partial scene.
#[derive(Clone, Debug, Serialize)]
pub struct Evaluation {
    /// Named mesh outputs.
    pub parts: Vec<Part>,
    /// Operators executed in this job, excluding cache hits.
    pub evaluated_nodes: usize,
    /// Cache hits in this job.
    pub cache_hits: usize,
    /// Retained cache payload bytes.
    pub cache_bytes: usize,
}
