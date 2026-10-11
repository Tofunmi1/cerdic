//! What interaction tells the outside: one line per event for the log, and the status report.

use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::system::Query;
use bevy_transform::components::Transform;
use serde_json::json;
use ve_interact::{Event, PointerId, TargetId};

use super::{Interactable, InteractionState, Label, entity_of, target_of};

fn label_of(target: TargetId, labels: &Query<&Label>) -> &'static str {
    entity_of(target)
        .and_then(|entity| labels.get(entity).ok())
        .map_or("?", |label| label.0)
}

/// One line describing an event, with objects by label.
pub(super) fn describe(event: &Event, labels: &Query<&Label>) -> String {
    match *event {
        Event::HoverStart { pointer, target } => {
            format!("hover {} on {}", pointer.0, label_of(target, labels))
        }
        Event::HoverEnd { pointer, target } => {
            format!("unhover {} off {}", pointer.0, label_of(target, labels))
        }
        Event::SelectStart { pointer, target } => {
            format!("press {} on {}", pointer.0, label_of(target, labels))
        }
        Event::SelectEnd {
            pointer,
            target,
            click,
        } => format!(
            "release {} on {}{}",
            pointer.0,
            label_of(target, labels),
            if click { " (click)" } else { "" }
        ),
        Event::GrabStart { pointer, target } => {
            format!("grab {} {}", pointer.0, label_of(target, labels))
        }
        Event::GrabEnd {
            pointer, target, ..
        } => format!("drop {} {}", pointer.0, label_of(target, labels)),
    }
}

/// What the status report shows about interaction: the objects, what is hovered and held, and the
/// most recent events.
pub(crate) fn report(
    state: &InteractionState,
    objects: &Query<(Entity, &Label, &Transform), With<Interactable>>,
) -> serde_json::Value {
    let name = |entity: Entity| objects.get(entity).map_or("?", |(_, label, _)| label.0);
    let hover: Vec<_> = (0..=3_u32)
        .filter_map(|id| {
            let target = state.machine.hover_of(PointerId(id))?;
            Some(json!({ "pointer": id, "target": name(entity_of(target)?) }))
        })
        .collect();
    let held: Vec<_> = state
        .machine
        .held_targets()
        .into_iter()
        .filter_map(entity_of)
        .map(|entity| {
            let holders: Vec<u32> = state
                .machine
                .holders_of(target_of(entity))
                .iter()
                .map(|p| p.0)
                .collect();
            json!({ "target": name(entity), "holders": holders })
        })
        .collect();
    let items: Vec<_> = objects
        .iter()
        .map(|(_, label, transform)| {
            json!({
                "label": label.0,
                "position": transform.translation.to_array(),
                "scale": transform.scale.x,
                "rotation": transform.rotation.to_array(),
            })
        })
        .collect();
    json!({ "hover": hover, "held": held, "objects": items, "events": state.log })
}
