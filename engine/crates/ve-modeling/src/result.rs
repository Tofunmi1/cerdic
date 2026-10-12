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
