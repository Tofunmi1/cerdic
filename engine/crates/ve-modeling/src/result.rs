//! Plain geometry output and explicit quality budgets.
use serde::{Deserialize, Serialize};
use ve_graph::NodeId;

pub use crate::scene_settings::{Haze, LightProbes, Wind};

const PREVIEW_FIELD_EDGE_SCALE: f64 = 2.0;

/// Maximum number of explicit and emissive-derived light outputs in one evaluated scene.
pub const MAX_SCENE_LIGHTS: usize = 128;

/// Requested tessellation/budget class; part of every cache and job key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    /// Interactive work: coarser field spacing for speed; no triangle limit.
    Preview,
    /// Final output at full detail; no triangle limit.
    Final,
}

impl Quality {
    /// Multiplier applied to authored field spacing before bounded grid preflight.
    pub(crate) const fn field_edge_scale(self) -> f64 {
        match self {
            Self::Preview => PREVIEW_FIELD_EDGE_SCALE,
            Self::Final => 1.0,
        }
    }

    /// Triangles a result may store: no limit (owner decision 2026-11-06). What really bounds a document is its memory
    /// ([`crate::budget::MEMORY_LIMIT`]), which is still checked, and the evaluation's own time and cancellation.
    pub(crate) fn triangles(self) -> usize {
        match self {
            Self::Preview | Self::Final => usize::MAX,
        }
    }

    /// Triangles a result may draw at full detail, every placement of a shared mesh counted: no limit either. The viewer's
    /// culling and detail levels decide what it really draws, and a scene heavier than [`DRAWN_TRIANGLES_WARNING`] still gets
    /// a warning so the author knows.
    pub(crate) fn drawn_triangles(self) -> usize {
        match self {
            Self::Preview | Self::Final => usize::MAX,
        }
    }
}

/// Past this many triangles drawn at full detail, an evaluation warns that the scene is heavy (`budget.drawn_heavy`).
pub(crate) const DRAWN_TRIANGLES_WARNING: usize = 4_000_000;

/// A bounded procedural shader recipe; no image assets or UVs are required.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterialRecipe {
    /// Uniform PBR shading.
    #[default]
    None,
    /// Longitudinal wood grain.
    Wood,
    /// Board seams and per-plank variation.
    Planks,
    /// Directional brushed metal.
    BrushedMetal,
    /// Fine leather pebble.
    Leather,
    /// Woven fabric.
    Fabric,
    /// Repeating mortar joints and brick variation.
    Brick,
    /// Low-contrast aggregate stone.
    Concrete,
    /// Veined marble.
    Marble,
    /// Subtle painted-wall grain.
    PaintedWall,
    /// Procedural water surface shading and vertex ripple.
    Water,
}

/// Cel shading set by a `solid.toon` node: light falls into flat bands with a tinted shadow colour.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Toon {
    /// Steps between the shadow colour and the lit colour (1 is two-tone, up to 4).
    pub bands: u32,
    /// Softness of each band edge, 0 hard to 1 soft.
    pub feather: f32,
    /// Moves the band edges toward the light (positive) or away from it, -1 to 1.
    pub shift: f32,
    /// Size of the hard highlight, 0 for none.
    pub spec: f32,
    /// What the surface colour is multiplied by in shadow (sRGB).
    pub shade_color: [f32; 3],
    /// Rim light strength on the lit side, 0 to 1.
    pub rim: f32,
    /// Outline thickness in screen pixels at any distance; 0 draws none.
    #[serde(default)]
    pub outline: f32,
    /// Outline colour (sRGB).
    #[serde(default = "default_outline_color")]
    pub outline_color: [f32; 3],
}

fn default_outline_color() -> [f32; 3] {
    [0.1, 0.06, 0.14]
}

pub(crate) use crate::finish::Finish;

#[path = "result/foliage.rs"]
mod foliage;
pub use foliage::Foliage;

#[path = "result/material.rs"]
mod material;
pub use material::Material;

/// The authored kind of a scene light.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneLightKind {
    /// Omnidirectional point source.
    Point,
    /// Directional cone with inner and outer falloff angles.
    Spot,
    /// Infinitely distant directional source.
    Sun,
}

/// A bounded, serializable scene light produced by a `light.*` graph operator.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneLight {
    /// Stable authored graph node.
    pub node: NodeId,
    /// Output label.
    pub name: String,
    /// Point, spot or sun.
    pub kind: SceneLightKind,
    /// World position for point and spot lights.
    pub position: [f32; 3],
    /// Direction the spot cone or sun rays travel.
    pub direction: [f32; 3],
    /// sRGB light colour.
    pub color: [f32; 3],
    /// Bounded intensity.
    pub intensity: f32,
    /// Falloff distance in metres for point and spot lights.
    pub range: f32,
    /// Full angle for the inner spot cone, in radians.
    pub inner_angle: f32,
    /// Full angle for the outer spot cone, in radians.
    pub outer_angle: f32,
}

/// A bounded, serializable scene environment produced by a `scene.environment` graph operator.
///
/// The sky and ground colours are consumed as a first-order hemisphere: an approximation of an
/// environment with no inter-surface light. Setting `gi` replaces it with traced light probes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneEnvironment {
    /// Stable authored graph node.
    pub node: NodeId,
    /// sRGB diffuse light arriving from above.
    pub sky_color: [f32; 3],
    /// sRGB diffuse light arriving from below.
    pub ground_color: [f32; 3],
    /// Ambient brightness multiplier.
    pub intensity: f32,
    /// Studio-sky reflection strength for metals and gloss, 0 to 1.
    pub reflection: f32,
    /// sRGB colour of the optional ground bounce.
    pub bounce_color: [f32; 3],
    /// Ground-bounce strength on downward-facing surfaces, 0 to 1.
    pub bounce: f32,
    /// Optional procedural sky background; a separate scene.environment may override its ambient terms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sky: Option<SceneSky>,
    /// Light the scene with traced light probes (global illumination) instead of the constant ambient above.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gi: Option<LightProbes>,
    /// The wind that bends foliage (see `solid.foliage`).
    #[serde(default)]
    pub wind: Wind,
    /// Distance haze and sunlight shafts.
    #[serde(default)]
    pub haze: Haze,
}

/// Analytic procedural sky authored by `scene.sky`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneSky {
    /// Stable source node.
    pub node: NodeId,
    /// Unit direction travelled by sunlight.
    pub sun_direction: [f32; 3],
    /// Direct sunlight intensity.
    pub sun_intensity: f32,
    /// Analytic atmosphere turbidity, from 1 to 10.
    pub turbidity: f32,
    /// sRGB horizon colour.
    pub horizon_color: [f32; 3],
    /// sRGB zenith colour.
    pub zenith_color: [f32; 3],
    /// sRGB ground colour.
    pub ground_color: [f32; 3],
    /// Whether procedural cloud bands are enabled.
    pub clouds: bool,
    /// Cloud amount from zero to one.
    pub cloud_cover: f32,
    /// Deterministic procedural cloud seed.
    pub seed: u32,
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
    /// Optional authored UVs, one pair per triangle corner in index order.
    #[serde(skip)]
    pub uvs: Vec<[f32; 2]>,
    /// Scene-baked ambient occlusion per source vertex, from 0 (occluded) to 1 (open sky).
    #[serde(skip)]
    pub occlusion: Vec<f32>,
    /// Explicit unit normals, one per vertex in `positions` order, for a part whose shading must not depend on the mesh alone (a terrain
    /// tile, whose edge normals come from the neighbouring ground). Empty means the viewer derives its own.
    #[serde(skip)]
    pub normals: Vec<[f32; 3]>,
    /// Per-vertex data in `positions` order, drawn as vertex colour (water: flow x and z remapped to 0..1, depth, foam); empty for none.
    #[serde(skip)]
    pub colors: Vec<[f32; 4]>,
    /// Volume of the evaluated solid.
    pub volume: f64,
    /// The surface given by the nearest `solid.material`, if any.
    pub material: Option<Material>,
    /// Named proper rigid sockets; prototype coordinates when placements are present.
    pub frames: std::collections::BTreeMap<String, crate::Frame>,
    /// Keyframed transform tracks from `anim.track` nodes (empty for a part that does not move).
    pub animation: Vec<crate::anim::Track>,
    /// Authored animation state machine, carried as data for the play runtime.
    #[serde(default)]
    pub state_machine: Option<ve_animstate::StateMachine>,
    /// The response from a `behavior.on` node (None for a part that does not react).
    pub behavior: Option<crate::anim::Behavior>,
    /// The output name of the part this one is attached to; it moves with that part (None for a free part).
    pub parent: Option<String>,
    /// The rest segment of a bone part (from `rig.bone`); None for an ordinary part.
    pub bone: Option<crate::rig::Segment>,
    /// How this part is skinned to bone parts, with per-vertex weights (from `rig.skin`); None for a rigid part.
    pub skin: Option<crate::rig::Skinning>,
    /// Editable shape keys (glTF morph targets), stored as per-vertex position deltas.
    #[serde(default)]
    pub morph_targets: Vec<MorphTarget>,
    /// The chain of bones this part is the target of (from `rig.ik`, with the bones' segments); None otherwise.
    pub ik: Option<crate::rig::IkChain>,
    /// The particle effect this part emits (from `fx.emitter` and the nodes that refine it); None otherwise.
    pub fx: Option<crate::fx::PartFx>,
    /// The ragdoll this part triggers (from `phys.ragdoll`, resolved against the bones); None otherwise.
    pub ragdoll: Option<crate::ragdoll::RagdollSpec>,
    /// The live-cloth description of this part (from `cloth.sim`, resolved against the bones); None for a part that is not cloth.
    pub cloth: Option<crate::cloth_sim::ClothSpec>,
    /// The live-fluid description of this part (from `fluid.sim`, resolved against the bones); None for a part that is not a liquid.
    pub fluid: Option<crate::fluid_sim::FluidSpec>,
    /// The simulation program of this part (from `sim.program`); None for a part that is not driven by one.
    pub sim: Option<crate::sim_program::SimSpec>,
    /// The physics properties of this part (from `phys.body`); None for a part that is not simulated.
    pub body: Option<crate::physics::PhysBody>,
    /// Column-major positive-determinant render placements; None means an ordinary solid.
    #[serde(skip)]
    pub transforms: Option<Vec<[f32; 16]>>,
    /// Camera distance band in metres for drawing placed copies; `None` means no range. Not serialized.
    #[serde(skip)]
    pub visible_range: Option<(f32, f32)>,
    /// Camera distance cross-fade width in metres for placed copies. Not serialized.
    #[serde(skip)]
    pub visible_fade: f32,
    /// Original placement indices, retained when reflected prototypes are separated.
    #[serde(skip)]
    pub instance_indices: Vec<u32>,
    /// Which material region of its output this part draws: 0 for the first (and for a single-material
    /// solid), 1, 2 and so on for the further colours of one fused solid. Parts of one output share
    /// `node` and `name`; the pair (`node`, `region`) identifies the surface to draw them with.
    pub region: u32,
    /// Maps baked onto this part's UVs by `bake.maps`; None for a part without any.
    #[serde(skip)]
    pub baked: Option<std::sync::Arc<BakedMaps>>,
}

/// One named shape key relative to a part's neutral mesh.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MorphTarget {
    /// Stable authored key name.
    pub name: String,
    /// Default glTF weight used when the part is loaded.
    pub weight: f32,
    /// Per-vertex offsets from the neutral mesh; transferred separately from JSON by the worker.
    #[serde(skip)]
    pub positions: Vec<[f32; 3]>,
}

/// Images baked onto a part's UV layout: square RGBA8, row 0 at v = 0.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BakedMaps {
    /// Side of each image, in pixels.
    pub size: u32,
    /// Tangent-space normal map (OpenGL convention), `size * size * 4` bytes, when baked.
    pub normal: Option<Vec<u8>>,
    /// Grey ambient occlusion (255 is open sky), `size * size * 4` bytes, when baked.
    pub ao: Option<Vec<u8>>,
}

/// A successful complete evaluation. Failures never yield a partial scene.
#[derive(Clone, Debug, Serialize)]
pub struct Evaluation {
    /// Named mesh outputs.
    pub parts: Vec<Part>,
    /// Scene lights, separate from geometry outputs.
    pub lights: Vec<SceneLight>,
    /// The scene environment, if one was authored; separate from geometry outputs.
    pub environment: Option<SceneEnvironment>,
    /// Saved scripts for the page to start, restart or stop.
    #[serde(default)]
    pub scripts: Vec<crate::ScriptDef>,
    /// The collected deterministic game definition, if a `game.collect` node is output.
    #[serde(skip)]
    pub game: Option<ve_game::GameDef>,
    /// Operators executed in this job, excluding cache hits.
    pub evaluated_nodes: usize,
    /// Cache hits in this job.
    pub cache_hits: usize,
    /// Retained cache payload bytes.
    pub cache_bytes: usize,
    /// Non-fatal operator reports, including attributes intentionally dropped by geometry nodes.
    pub warnings: Vec<EvaluationWarning>,
}

/// A typed, non-fatal report attached to an authored graph node.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct EvaluationWarning {
    /// Stable diagnostic code.
    pub code: String,
    /// Authored node ID that produced the warning.
    pub node: String,
    /// Related authored nodes, when a diagnostic links multiple parts.
    pub related: Vec<String>,
    /// Human-readable detail.
    pub message: String,
}

impl EvaluationWarning {
    /// Reports that concave n-gons were triangulated to make Catmull-Clark input well-defined.
    #[must_use]
    pub fn concave_face_triangulated(node: impl Into<String>, count: usize) -> Self {
        Self {
            code: "poly.subdivide.concave_face_triangulated".to_owned(),
            node: node.into(),
            related: Vec::new(),
            message: format!("Triangulated {count} concave face(s) before subdivision"),
        }
    }

    /// Builds the stable report emitted when subdivision omits a discrete integer point attribute.
    #[must_use]
    pub fn attribute_dropped(node: impl Into<String>, name: &str) -> Self {
        Self {
            code: "poly.subdivide.attribute_dropped".to_owned(),
            node: node.into(),
            related: Vec::new(),
            message: format!("Integer point attribute '{name}' was dropped during subdivision"),
        }
    }
}
