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
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// Selects `node` (or clears the selection) and the viewport parts that light up with it.
pub(crate) fn set(node: Option<String>, parts: BTreeSet<String>) {
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        if state.node != node || state.parts != parts {
            state.node = node;
            state.parts = parts;
            state.generation = state.generation.wrapping_add(1);
        }
    });
}

/// Whether the viewport part built by `node` is highlighted as selected.
pub(crate) fn is_part(node: &str) -> bool {
    STATE.with(|cell| cell.borrow().parts.contains(node))
}

/// The selection for the page: `{ node, parts, generation }`.
pub(crate) fn to_json() -> String {
    STATE.with(|cell| {
        let state = cell.borrow();
        json!({"node": state.node, "parts": state.parts, "generation": state.generation})
            .to_string()
    })
}

/// A click on a part of the evaluated model selects the node that builds it.
pub(crate) fn select_on_click(
    mut interaction: ResMut<InteractionState>,
    authored: Query<&Authored>,
) {
    for entity in interaction.take_clicked() {
        if let Ok(part) = authored.get(entity) {
            set(
                Some(part.node().to_owned()),
                BTreeSet::from([part.node().to_owned()]),
            );
        }
    }
}
