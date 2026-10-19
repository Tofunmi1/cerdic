//! Bounded geometry evaluation shared by native tools and the renderer-free browser worker.
pub mod anim;
mod ao;
mod appearance;
mod assembly;
mod bake;
mod bezier;
mod budget;
mod cache;
pub mod creature;
mod curves;
mod deform;
pub mod easing;
mod emitters;
mod environment;
mod evaluate;
mod field_ops;
pub mod flags;
mod foliage;
mod frames;
pub mod hierarchy;
mod hulls;
pub mod ik;
mod instances;
mod job;
mod kits;
mod lights;
mod noise;
mod operations;
pub mod physics;
mod plan;
mod poly_ops;
mod profile_ops;
mod reaction;
mod regions;
mod result;
pub mod rig;
mod shapes;
mod style;
mod terrain;
mod values;
mod wave;

pub use evaluate::Evaluator;
pub use frames::Frame;
pub use job::{JobToken, hash};
pub use kits::{KITS, Kit};
pub use poly_ops::suggestion as poly_suggestion;
pub use result::{
    BakedMaps, Evaluation, EvaluationWarning, Foliage, LightProbes, MAX_SCENE_LIGHTS, Material,
    MaterialRecipe, Part, Quality, SceneEnvironment, SceneLight, SceneLightKind, Wind,
};

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
