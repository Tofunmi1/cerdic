//! The declared UI in the world (ADR 0074): layout queries for the host, focus, and the buttons' events.
//!
//! The host presses the reserved names `ui_next`, `ui_prev` and `ui_accept` (the page binds them to keys and the d-pad) and `ui:ID` for a
//! click on a button. Focus moves through the visible buttons in declaration order and wraps; accepting a button raises the event `ui:ID`
//! and runs the button's own `do=` actions. They work in frozen states too (a title or a pause menu is made of buttons).
use std::collections::{BTreeMap, BTreeSet};

use super::{Event, World};
use crate::ui::layout::{self, Context};
use crate::{Effect, GameError, UiBox};

impl World {
    fn ui_context<'a>(&'a self, hidden: &'a BTreeSet<String>, aspect: f64) -> Context<'a> {
        Context {
            hidden,
            camera: &self.camera_axes,
            floaters: &self.floaters,
            aspect,
            def: &self.def,
            state: &self.flow.state,
            vars: &self.vars,
            labels: &self.ui_labels,
            focus: &self.flow.focus,
            entities: &self.entities,
        }
    }

    /// Runs `work` with the layout context: the elements whose `when=` is false are worked out first and left out of it.
    fn with_ui_context<R>(&self, aspect: f64, work: impl FnOnce(&Context) -> R) -> R {
        let none = BTreeSet::new();
        let hidden = layout::hidden_by_when(&self.def, &self.ui_context(&none, aspect));
        work(&self.ui_context(&hidden, aspect))
    }

    /// Every visible UI element laid out for a screen of this aspect ratio (width over height), in screen heights, containers before
    /// their children; empty when the game declares no UI.
    #[must_use]
    pub fn ui_layout(&self, aspect: f64) -> Vec<UiBox> {
        if self.def.ui.is_empty() {
            return Vec::new();
        }
        let aspect = sanitise(aspect);
        self.with_ui_context(aspect, |cx| layout::layout(&self.def, cx, aspect))
    }

    /// The button under a point (in the same units as [`World::ui_layout`]), if any.
    #[must_use]
    pub fn ui_hit(&self, aspect: f64, x: f64, y: f64) -> Option<String> {
        layout::hit(&self.ui_layout(aspect), x, y)
    }

    /// What the page calls each action on the active device, for `{key:ACTION}` in texts (`jump` is `Space` on a keyboard, `A` on a pad).
    /// Not part of the game's state: it is the host's wording.
    pub fn set_ui_labels(&mut self, labels: BTreeMap<String, String>) {
        self.ui_labels = labels;
    }

    /// The declared UI and the scripts' elements as one drawing frame (JSON `{aspect, items}`).
    pub(super) fn declared_ui_specs(&self, aspect: f64) -> Vec<(String, serde_json::Value)> {
        let boxes = self.ui_layout(aspect);
        layout::specs(&self.def, &boxes)
    }

    /// Reads the host's UI presses from `pressed`: focus moves, a button is accepted or clicked.
    pub(super) fn ui_input(&mut self, pressed: &[String], queue: &mut Vec<Event>, effects: &mut Vec<Effect>) -> Result<(), GameError> {
        if self.def.ui.is_empty() {
            return Ok(());
        }
        for name in pressed {
            let buttons: Vec<String> = self.with_ui_context(16.0 / 9.0, |cx| layout::visible_buttons(&self.def, cx).iter().map(|b| b.id.clone()).collect());
            if buttons.is_empty() {
                continue;
            }
            let at = self
                .with_ui_context(16.0 / 9.0, |cx| layout::effective_focus(&self.def, cx, &self.flow.focus))
                .and_then(|id| buttons.iter().position(|b| *b == id))
                .unwrap_or(0);
            match name.as_str() {
                "ui_next" => self.flow.focus.clone_from(&buttons[(at + 1) % buttons.len()]),
                "ui_prev" => self.flow.focus.clone_from(&buttons[(at + buttons.len() - 1) % buttons.len()]),
                "ui_accept" => self.press_button(&buttons[at].clone(), queue, effects)?,
                other => {
                    if let Some(id) = other.strip_prefix("ui:")
                        && buttons.iter().any(|b| b == id)
                    {
                        self.press_button(id, queue, effects)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn press_button(&mut self, id: &str, queue: &mut Vec<Event>, effects: &mut Vec<Effect>) -> Result<(), GameError> {
        id.clone_into(&mut self.flow.focus);
        let event = Event { name: format!("ui:{id}"), a: None, b: None, depth: 0 };
        queue.push(event.clone());
        let actions = self.def.ui.iter().find(|item| item.id == id).map(crate::UiDef::actions).unwrap_or_default();
        for action in actions {
            self.action(&action, &event, queue, effects)?;
        }
        Ok(())
    }
}

fn sanitise(aspect: f64) -> f64 {
    if aspect.is_finite() && aspect > 0.2 { aspect } else { 16.0 / 9.0 }
}
