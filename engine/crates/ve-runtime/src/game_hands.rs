//! Hands for the live game: where the player's hands are and whether each grip and trigger is squeezed, in a headset and on the desktop.
//!
//! In a presenting headset a hand is a tracked controller: its grip pose (placed by the tracking origin) and its trigger (button 0) and
//! squeeze (button 1). On the desktop the right hand floats in front of the camera and the page sets its grip and trigger from keys. A host
//! (a test, a recording) can instead hand in every hand's pose and buttons with [`set_override`].
use std::cell::{Cell, RefCell};

use bevy_math::Vec3;
use serde_json::Value;
use ve_game::{HandInput, HandRotation, HandSide};
use ve_xr_web::frame::Handedness;
use ve_xr_web::{SessionState, XrInput, XrRig, session_state};

/// A button counts as pressed past this analogue value.
const PRESSED: f32 = 0.5;
/// Where the desktop hand floats relative to the eye: forward, right and down, in metres.
const DESKTOP_REACH: Vec3 = Vec3::new(0.25, -0.2, -0.65);

thread_local! {
    static OVERRIDE_ROTATIONS: RefCell<Vec<HandRotation>> = const { RefCell::new(Vec::new()) };
    static OVERRIDE: RefCell<Option<Vec<HandInput>>> = const { RefCell::new(None) };
    static DESKTOP: Cell<(bool, bool)> = const { Cell::new((false, false)) };
}

/// Sets the desktop hand's grip and trigger (the page calls this from key events).
pub(crate) fn set_desktop(grip: bool, trigger: bool) {
    DESKTOP.with(|cell| cell.set((grip, trigger)));
}

/// Replaces every hand with `[{"side": "left"|"right", "position": [x, y, z], "grip": bool, "trigger": bool}, ...]`; an empty string goes
/// back to the controllers (or the desktop hand). Returns false for text that is not that.
pub(crate) fn set_override(json: &str) -> bool {
    if json.trim().is_empty() {
        OVERRIDE.with(|cell| *cell.borrow_mut() = None);
        OVERRIDE_ROTATIONS.with(|cell| cell.borrow_mut().clear());
        return true;
    }
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(json) else {
        return false;
    };
    let hands: Option<Vec<HandInput>> = items.iter().map(parse_hand).collect();
    match hands {
        Some(hands) => {
            // An optional `"rotation": [x, y, z, w]` turns the hand (a club points along its -Z axis).
            let rotations = items.iter().filter_map(parse_rotation).collect();
            OVERRIDE.with(|cell| *cell.borrow_mut() = Some(hands));
            OVERRIDE_ROTATIONS.with(|cell| *cell.borrow_mut() = rotations);
            true
        }
        None => false,
    }
}

fn parse_rotation(item: &Value) -> Option<HandRotation> {
    let side = match item["side"].as_str()? {
        "left" => HandSide::Left,
        "right" => HandSide::Right,
        _ => return None,
    };
    let q = item["rotation"].as_array()?;
    let rotation = [
        q.first()?.as_f64()?,
        q.get(1)?.as_f64()?,
        q.get(2)?.as_f64()?,
        q.get(3)?.as_f64()?,
    ];
    rotation
        .iter()
        .all(|v| v.is_finite())
        .then_some(HandRotation { side, rotation })
}

/// The rotation of each tracked hand this frame (a hand not listed is not turned): a controller's own, or the view's for the desktop hand.
pub(crate) fn rotations(input: &XrInput, rig: &XrRig) -> Vec<HandRotation> {
    if OVERRIDE.with(|cell| cell.borrow().is_some()) {
        return OVERRIDE_ROTATIONS.with(|cell| cell.borrow().clone());
    }
    if session_state() == SessionState::Presenting {
        return input
            .controllers
            .iter()
            .filter_map(|controller| {
                let grip = controller.grip?;
                let side = match controller.handedness {
                    Handedness::Left => HandSide::Left,
                    Handedness::Right => HandSide::Right,
                    Handedness::None => return None,
                };
                let q = bevy_math::Quat::from_mat4(&(rig.0 * grip)).normalize();
                Some(HandRotation {
                    side,
                    rotation: [q.x, q.y, q.z, q.w].map(f64::from),
                })
            })
            .collect();
    }
    let Some((_, rotation)) = crate::viewer::pose() else {
        return Vec::new();
    };
    vec![HandRotation {
        side: HandSide::Right,
        rotation: [rotation.x, rotation.y, rotation.z, rotation.w].map(f64::from),
    }]
}

fn parse_hand(item: &Value) -> Option<HandInput> {
    let side = match item["side"].as_str()? {
        "left" => HandSide::Left,
        "right" => HandSide::Right,
        _ => return None,
    };
    let position = item["position"].as_array()?;
    let [x, y, z] = [
        position.first()?.as_f64()?,
        position.get(1)?.as_f64()?,
        position.get(2)?.as_f64()?,
    ];
    Some(HandInput {
        side,
        position: [x, y, z],
        grip: item["grip"].as_bool().unwrap_or(false),
        trigger: item["trigger"].as_bool().unwrap_or(false),
    })
}

/// The hands this frame.
pub(crate) fn hands(input: &XrInput, rig: &XrRig) -> Vec<HandInput> {
    if let Some(hands) = OVERRIDE.with(|cell| cell.borrow().clone()) {
        return hands;
    }
    if session_state() == SessionState::Presenting {
        return input
            .controllers
            .iter()
            .filter_map(|controller| {
                let grip = controller.grip?;
                let position = (rig.0 * grip).w_axis.truncate();
                Some(HandInput {
                    side: match controller.handedness {
                        Handedness::Left => HandSide::Left,
                        Handedness::Right => HandSide::Right,
                        Handedness::None => return None,
                    },
                    position: position.to_array().map(f64::from),
                    grip: controller.buttons[1] > PRESSED,
                    trigger: controller.buttons[0] > PRESSED,
                })
            })
            .collect();
    }
    let Some((eye, rotation)) = crate::viewer::pose() else {
        return Vec::new();
    };
    let (grip, trigger) = DESKTOP.with(Cell::get);
    vec![HandInput {
        side: HandSide::Right,
        position: (eye + rotation * DESKTOP_REACH).to_array().map(f64::from),
        grip,
        trigger,
    }]
}
