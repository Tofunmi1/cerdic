//! Data-driven deterministic game rules with no engine or platform dependencies.
mod camera;
mod effect;
mod error;
mod event;
mod expr;
mod ground;
mod parse;
pub mod playtest;
#[cfg(test)]
mod rolling_tests;
mod ui;
mod world;

pub use camera::{CameraDef, CameraInput, CameraPose, CameraRig};
pub use effect::{Effect, EffectKind};
pub use error::{Diagnostic, GameError};
pub use event::HostEvent;
pub use event::{HandInput, HandRotation, HandSide};
pub use event::{Inputs, Outcome, Status};
pub use ground::GroundField;
pub use parse::LIBRARIES as SCRIPT_LIBRARIES;
pub use parse::agent::{AgentDef, NavDef};
pub use parse::combat::{HealthDef, ProjectileDef, WeaponDef};
pub use parse::controller::ControllerDef;
pub use parse::party::{AbilityDef, BrainDef, TargetingDef};
pub use parse::rolling::{SurfaceDef, TerrainDef};
pub use parse::{GameDef, Goal, HudDef, InputDef, KindDef, Rule, ScriptDef, SpawnDef, VariableDef};
pub use ui::{UiBox, UiDef};
pub use world::{Entity, Snapshot, World};
