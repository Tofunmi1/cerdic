//! Pointer input: DOM events are queued, then drained into Bevy messages once per frame.

use std::cell::RefCell;
use std::rc::Rc;

use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use bevy_input::ButtonState;
use bevy_input::mouse::{MouseButton, MouseButtonInput, MouseScrollUnit, MouseWheel};
use bevy_input::touch::TouchPhase;
use bevy_math::Vec2;
use bevy_window::{CursorLeft, CursorMoved};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;
use web_sys::{Event, EventTarget, MouseEvent, PointerEvent, WheelEvent};

/// A DOM event reduced to what the engine needs, in CSS (logical) pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum RawInput {
    PointerMoved {
        x: f32,
        y: f32,
    },
    /// The pointer left the canvas (and is not captured), so there is no cursor over the scene.
    PointerLeft,
    Button {
        button: i16,
        pressed: bool,
    },
    Wheel {
        x: f32,
        y: f32,
        lines: bool,
    },
}

/// Events waiting to be delivered on the next frame.
pub(crate) type Queue = Rc<RefCell<Vec<RawInput>>>;

struct Listener {
    target: EventTarget,
    name: &'static str,
    closure: Closure<dyn FnMut(Event)>,
}

impl Drop for Listener {
    fn drop(&mut self) {
        // Failure to remove during teardown is harmless: the closure is dropped right after.
        let _ = self
            .target
            .remove_event_listener_with_callback(self.name, self.closure.as_ref().unchecked_ref());
    }
}

/// Active DOM listeners; dropping this detaches them all.
pub(crate) struct Listeners {
    _items: Vec<Listener>,
}

fn add(
    items: &mut Vec<Listener>,
    target: &EventTarget,
    name: &'static str,
    handler: impl FnMut(Event) + 'static,
) {
    let closure = Closure::<dyn FnMut(Event)>::new(handler);
    if target
        .add_event_listener_with_callback(name, closure.as_ref().unchecked_ref())
        .is_ok()
    {
        items.push(Listener {
            target: target.clone(),
            name,
            closure,
        });
    }
}

impl Listeners {
    /// Attaches pointer, wheel and context-menu listeners to `target`, feeding `queue`.
    pub(crate) fn attach(target: &EventTarget, queue: &Queue) -> Self {
        let mut items = Vec::new();

        let q = Rc::clone(queue);
        add(&mut items, target, "pointermove", move |event| {
            if let Some(e) = event.dyn_ref::<PointerEvent>() {
                #[expect(clippy::cast_precision_loss, reason = "CSS pixel offsets are small")]
                let (x, y) = (e.offset_x() as f32, e.offset_y() as f32);
                q.borrow_mut().push(RawInput::PointerMoved { x, y });
            }
        });

        // Inside an editor the canvas is one panel among others: without this the hover from the
        // last position over the canvas would stay lit while the pointer is on another panel.
        let q = Rc::clone(queue);
        add(&mut items, target, "pointerleave", move |_| {
            q.borrow_mut().push(RawInput::PointerLeft);
        });

        for (name, pressed) in [("pointerdown", true), ("pointerup", false)] {
            let q = Rc::clone(queue);
            add(&mut items, target, name, move |event| {
                if let Some(e) = event.dyn_ref::<PointerEvent>() {
                    if pressed
                        && let Some(element) = e
                            .target()
                            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                    {
                        // Keep receiving moves and the release if a drag ends outside the canvas,
                        // so a grab can never be left held. Refusal (no such pointer) is harmless.
                        let _ = element.set_pointer_capture(e.pointer_id());
                    }
                    q.borrow_mut().push(RawInput::Button {
                        button: e.button(),
                        pressed,
                    });
                }
            });
        }

        let q = Rc::clone(queue);
        add(&mut items, target, "wheel", move |event| {
            if let Some(e) = event.dyn_ref::<WheelEvent>() {
                event.prevent_default();
                #[expect(clippy::cast_possible_truncation, reason = "wheel deltas fit f32")]
                let (x, y) = (e.delta_x() as f32, e.delta_y() as f32);
                q.borrow_mut().push(RawInput::Wheel {
                    x,
                    y,
                    lines: e.delta_mode() == WheelEvent::DOM_DELTA_LINE,
                });
            }
        });

        // The browser context menu would swallow right-clicks.
        add(&mut items, target, "contextmenu", |event| {
            if event.dyn_ref::<MouseEvent>().is_some() {
                event.prevent_default();
            }
        });

        Self { _items: items }
    }
}

fn mouse_button(index: i16) -> Option<MouseButton> {
    match index {
        0 => Some(MouseButton::Left),
        1 => Some(MouseButton::Middle),
        2 => Some(MouseButton::Right),
        3 => Some(MouseButton::Back),
        4 => Some(MouseButton::Forward),
        _ => None,
    }
}

/// Delivers queued events to the world as Bevy messages for `window`. `last` tracks the previous
/// cursor position so `CursorMoved::delta` is correct across frames.
pub(crate) fn drain(world: &mut World, window: Entity, queue: &Queue, last: &mut Option<Vec2>) {
    let events = std::mem::take(&mut *queue.borrow_mut());
    for event in events {
        match event {
            RawInput::PointerMoved { x, y } => {
                let position = Vec2::new(x, y);
                let delta = last.map(|previous| position - previous);
                *last = Some(position);
                world.write_message(CursorMoved {
                    window,
                    position,
                    delta,
                });
            }
            RawInput::PointerLeft => {
                *last = None;
                world.write_message(CursorLeft { window });
            }
            RawInput::Button { button, pressed } => {
                if let Some(button) = mouse_button(button) {
                    let state = if pressed {
                        ButtonState::Pressed
                    } else {
                        ButtonState::Released
                    };
                    world.write_message(MouseButtonInput {
                        button,
                        state,
                        window,
                    });
                }
            }
            RawInput::Wheel { x, y, lines } => {
                world.write_message(MouseWheel {
                    unit: if lines {
                        MouseScrollUnit::Line
                    } else {
                        MouseScrollUnit::Pixel
                    },
                    x,
                    y,
                    window,
                    phase: TouchPhase::Moved,
                });
            }
        }
    }
}
