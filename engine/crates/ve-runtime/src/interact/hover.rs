//! The glow on hovered and selected objects: it fades in and out, and the original colour comes back
//! exactly.

use bevy_color::{Color, LinearRgba};
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::system::{Commands, Query, Res};
use bevy_time::{Real, Time};
use ve_render::VeMaterial;

use super::{Interactable, InteractionState};
use crate::projection::Authored;
use crate::selection;

/// How much of an object's colour is added as glow while it is hovered, for objects that can be
/// grabbed, and for authored geometry that can only be selected (a gentler cue: a whole model
/// brightening sharply as the pointer crosses it reads as flicker).
const HOVER_GLOW: f32 = 0.35;
const SELECT_GLOW: f32 = 0.10;

/// The steady glow of a selected part, on top of any hover glow.
const SELECTED_GLOW: f32 = 0.30;

/// How quickly the glow follows the pointer or the selection, per second (about 70 ms to settle).
const HOVER_FADE_RATE: f32 = 14.0;

/// Marks an object whose emissive colour is raised: the original colour, and how far the hover glow
/// and the selection glow have faded in (each 0 to 1).
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct HoverTint {
    original: Color,
    level: f32,
    selected_level: f32,
}

/// Fades the emissive colour of hovered and selected objects up and back down, restoring it exactly
/// afterwards.
pub(crate) fn highlight_hover(
    mut commands: Commands,
    state: Res<InteractionState>,
    time: Res<Time<Real>>,
    mut objects: Query<(
        Entity,
        &mut VeMaterial,
        &Interactable,
        Option<&Authored>,
        Option<&mut HoverTint>,
    )>,
) {
    let ease = 1.0 - (-HOVER_FADE_RATE * time.delta_secs()).exp();
    for (entity, mut material, interactable, authored, tint) in &mut objects {
        let hovered = state.hovered.contains(&entity);
        let selected = authored.is_some_and(|part| selection::is_part(part.node()));
        let Some(mut tint) = tint else {
            if hovered || selected {
                commands.entity(entity).insert(HoverTint {
                    original: material.emissive,
                    level: 0.0,
                    selected_level: 0.0,
                });
            }
            continue;
        };
        tint.level += (f32::from(u8::from(hovered)) - tint.level) * ease;
        tint.selected_level += (f32::from(u8::from(selected)) - tint.selected_level) * ease;
        if !hovered && !selected && tint.level < 0.01 && tint.selected_level < 0.01 {
            material.emissive = tint.original;
            commands.entity(entity).remove::<HoverTint>();
            continue;
        }
        let hover = if interactable.grabbable {
            HOVER_GLOW
        } else {
            SELECT_GLOW
        };
        let glow = hover * tint.level + SELECTED_GLOW * tint.selected_level;
        let (base, original) = (
            LinearRgba::from(material.base_color),
            LinearRgba::from(tint.original),
        );
        material.emissive = Color::linear_rgb(
            original.red + base.red * glow,
            original.green + base.green * glow,
            original.blue + base.blue * glow,
        );
    }
}
