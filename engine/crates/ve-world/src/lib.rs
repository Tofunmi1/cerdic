//! Pure-arithmetic world generation (ADR 0056): tile-aware terrain, erosion, and the modules that follow (scatter, flora, lod, river, rock).
//!
//! One crate with one module per family, not one crate each: they share conventions (world coordinates, seeds, cancellation) and none
//! depends on the graph, Bevy or the platform, so the node wiring in `ve-modeling` stays the only place that knows both sides.
pub mod bands;
pub mod biome;
pub mod building;
pub mod city;
pub mod cloth;
pub mod erosion;
pub mod expr;
pub mod flora;
pub mod fluid;
pub mod lod;
pub mod noise;
mod num;
mod pixel_font;
pub mod pixel_text;
pub mod river;
pub mod rooms;
pub mod scatter;
pub mod sim;
pub mod streetscape;
pub mod survey;
pub mod terrain;
pub mod terrain_lod;

pub mod water_sim;
