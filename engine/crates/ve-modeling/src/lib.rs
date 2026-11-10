//! Bounded geometry evaluation shared by native tools and the renderer-free browser worker.
pub mod anim;
mod anim_machine;
mod anim_retarget;
mod anim_simplify;
mod ao;
mod appearance;
mod assembly;
mod bake;
mod bezier;
mod budget;
mod cache;
mod check_motion;
pub mod cloth_sim;
pub mod creature;
mod curve_expr;
mod curve_fit;
mod curves;
mod deform;
mod drape;
pub mod easing;
mod emitters;
mod environment;
mod evaluate;
mod field_ops;
mod finish;
pub mod flags;
pub mod fluid_sim;
pub mod sim_program;
mod detail_panels;
mod rooms_plan;
mod sign_text;
mod foliage;
mod fracture;
mod frames;
pub mod fx;
mod game_nodes;
pub mod hierarchy;
mod hulls;
mod human;
mod human_eye;
mod human_hair;
mod human_rig;
pub mod ik;
pub mod ik_limb;
pub mod inspect;
mod instances;
mod job;
mod kits;
mod lights;
mod material_presets;
mod morph;
mod noise;
mod operations;
pub mod physics;
mod plan;
mod poly_ops;
mod profile_ops;
pub mod ragdoll;
mod reaction;
mod regions;
mod result;
pub mod rig;
mod scene_settings;
pub mod script;
mod script_nodes;
mod shapes;
mod style;
mod terrain;
mod terrain_look;
mod values;
mod volume;
mod water_look;
mod wave;
mod world_city;
mod world_field;
mod world_flora;
mod world_lake;
mod world_ops;
mod world_pad;
mod world_river;
mod world_scatter;
mod world_terrain;
mod world_water;

pub use evaluate::{Evaluator, NextNode};
pub use frames::Frame;
pub use human::{human_pack_loaded, set_human_pack};
pub use job::{JobToken, hash};
pub use kits::{KITS, Kit, MACRO_KITS, PLANT_KITS};
pub use poly_ops::suggestion as poly_suggestion;
pub use result::{
    BakedMaps, Evaluation, EvaluationWarning, Foliage, Haze, LightProbes, MAX_SCENE_LIGHTS,
    Material, MaterialRecipe, MorphTarget, Part, Quality, SceneEnvironment, SceneLight,
    SceneLightKind, Wind,
};
pub use script_nodes::ScriptDef;

pub(crate) type Fail = Box<ve_graph::Diagnostic>;

pub(crate) fn fail(code: &str, message: impl Into<String>) -> Fail {
    Box::new(ve_graph::Diagnostic::error(code, message))
}

/// Like [`fail`], for a refusal the caller can act on: `suggestion` names what to change.
pub(crate) fn fail_suggest(
    code: &str,
    message: impl Into<String>,
    suggestion: impl Into<String>,
) -> Fail {
    Box::new(ve_graph::Diagnostic::error(code, message).suggest(suggestion))
}
