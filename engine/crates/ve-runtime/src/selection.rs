//! The selected node: set by the page (the outliner) or by a click on a part in the viewport, read
//! by the highlight and by the page. Selection is view state, not document state: it is never saved
//! and never goes through a command. The page says which parts a node feeds (it holds the graph), so
//! picking an upstream node lights up what it builds.

use std::cell::RefCell;
use std::collections::BTreeSet;

use bevy_ecs::system::{Query, ResMut};
use serde_json::json;

use crate::interact::InteractionState;
use crate::projection::Authored;

#[derive(Default)]
struct State {
    node: Option<String>,
    parts: BTreeSet<String>,
    /// Counts changes, so the page can tell cheaply that the selection moved.
    generation: u32,
    instance: Option<u32>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// Selects `node` (or clears the selection) and the viewport parts that light up with it.
pub(crate) fn set(node: Option<String>, parts: BTreeSet<String>) {
    set_instance(node, parts, None);
}

fn set_instance(node: Option<String>, parts: BTreeSet<String>, instance: Option<u32>) {
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        if state.node != node || state.parts != parts || state.instance != instance {
            state.node = node;
            state.parts = parts;
            state.instance = instance;
            state.generation = state.generation.wrapping_add(1);
        }
    });
}

/// Whether the viewport part built by `node` is highlighted as selected.
pub(crate) fn is_part(node: &str, instance: Option<u32>) -> bool {
    STATE.with(|cell| {
        let state = cell.borrow();
        state.parts.contains(node) && state.instance.is_none_or(|i| Some(i) == instance)
    })
}

/// Drops a stale per-instance selection when a newly committed set is shorter or absent.
pub(crate) fn validate_instances(sets: &std::collections::BTreeMap<String, usize>) {
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        if state.instance.is_some_and(|index| {
            state
                .node
                .as_ref()
                .and_then(|id| sets.get(id))
                .is_none_or(|count| index as usize >= *count)
        }) {
            state.instance = None;
            state.generation = state.generation.wrapping_add(1);
        }
    });
}

/// The selection for the page: `{ node, parts, generation }`.
pub(crate) fn to_json() -> String {
    STATE.with(|cell| {
        let state = cell.borrow();
        json!({"node": state.node, "parts": state.parts, "generation": state.generation,"instance":state.instance})
            .to_string()
    })
}

/// A click on a part of the evaluated model selects the node that builds it.
pub(crate) fn select_on_click(
    mut interaction: ResMut<InteractionState>,
    authored: Query<(&Authored, Option<&crate::projection::InstanceIndex>)>,
    mut behaviors: ResMut<crate::behavior::Behaviors>,
) {
    for entity in interaction.take_clicked() {
        behaviors.clicked(entity);
        // Play mode is for using the scene, not editing it: a click acts and does not select.
        if behaviors.play {
            continue;
        }
        if let Ok((part, index)) = authored.get(entity) {
            set_instance(
                Some(part.node().to_owned()),
                BTreeSet::from([part.node().to_owned()]),
                index.map(|index| index.0),
            );
        }
    }
}
