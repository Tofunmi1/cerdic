//! The look of a water surface (ADR 0059): colour by depth, a flowing ripple pattern, foam. It carries no texture; the viewer draws it per
//! pixel from the flow, depth and foam the mesh carries as vertex colour.
use serde::{Deserialize, Serialize};

/// A water surface's look. Colours are sRGB; the other values are 0 to 1 unless noted.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WaterLook {
    /// Colour of clear shallow water.
    pub shallow: [f32; 3],
    /// Colour of deep water.
    pub deep: [f32; 3],
    /// Metres of depth over which shallow turns into deep.
    pub clarity: f32,
    /// Speed of the streaming surface pattern, 1 is natural, up to 4.
    pub flow: f32,
    /// Strength of the small waves, 0 to 2; zero gives a flat wave field.
    pub ripple: f32,
    /// Main wave travel direction in degrees; zero travels along positive X.
    #[serde(default)]
    pub direction: f32,
    /// Seed of the repeatable wave table.
    #[serde(default)]
    pub seed: u32,
    /// Amount of foam.
    pub foam: f32,
    /// Enables the interactive wave simulation.
    #[serde(default)]
    pub sim: bool,
    /// Cells along the longer side (32 to 256).
    #[serde(default = "default_resolution")]
    pub sim_resolution: u32,
    /// Velocity damping per second.
    #[serde(default = "default_damping")]
    pub sim_damping: f32,
    /// Fallback depth in metres.
    #[serde(default = "default_depth")]
    pub sim_depth: f32,
    /// Seeded drops per second per 100 square metres of water.
    #[serde(default)]
    pub rain: f32,
    /// Filled basins opt into the runtime's bounded 3D particle reference simulation.
    #[serde(default)]
    pub volumetric: bool,
}

/// The lake keeps its legacy colour and speed controls while sharing the wave look with rivers.
#[expect(
    clippy::cast_possible_truncation,
    reason = "graph parameters have bounded ranges"
)]
pub(crate) fn lake_look(
    node: &ve_graph::Node,
    color: [f32; 3],
    speed: f32,
) -> Result<WaterLook, crate::Fail> {
    use crate::profile_ops::{number, whole};
    let number =
        |name| number(node, name).map_err(|error| crate::operations::kernel_fail(node, &error));
    let (sim, sim_resolution, sim_damping, sim_depth, rain) = simulation(node);
    Ok(WaterLook {
        shallow: color,
        deep: [0.02, 0.12, 0.2],
        clarity: 3.0,
        flow: (number("flow")? as f32 * speed).clamp(0.0, 4.0),
        ripple: number("ripple")? as f32,
        foam: 0.5,
        sim,
        sim_resolution,
        sim_damping,
        sim_depth,
        rain,
        volumetric: false,
        direction: number("direction")?.to_degrees() as f32,
        seed: whole(node, "seed").map_err(|error| crate::operations::kernel_fail(node, &error))?,
    })
}

/// Simulation defaults also apply to old nodes and rivers without simulation controls.
#[expect(
    clippy::cast_possible_truncation,
    reason = "graph ranges bound physical water parameters"
)]
pub(crate) fn simulation(node: &ve_graph::Node) -> (bool, u32, f32, f32, f32) {
    let number = |key, default| {
        node.params
            .get(key)
            .and_then(ve_graph::Value::as_f64)
            .unwrap_or(default) as f32
    };
    (
        matches!(node.params.get("sim"), Some(ve_graph::Value::Bool(true))),
        crate::profile_ops::whole(node, "sim_resolution").unwrap_or(128),
        number("sim_damping", 0.6),
        number("sim_depth", 1.0),
        number("rain", 0.0),
    )
}

fn default_resolution() -> u32 {
    128
}
fn default_damping() -> f32 {
    0.6
}
fn default_depth() -> f32 {
    1.0
}
