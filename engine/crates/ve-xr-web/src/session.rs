//! XR session lifecycle: request, per-frame data collection and end. Browser only.

use std::cell::RefCell;

use bevy_math::Mat4;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::{Closure, JsValue};
use web_sys::HtmlCanvasElement;

use crate::bridge::{self, XrBridge};
use crate::frame::{XrFrame, XrView, parse_controllers};
use crate::state::{
    SessionState, publish_frame, session_state, set_last_error, set_refresh_hz, set_session_state,
    set_viewport_scale_supported, viewport_scale,
};

/// Why a session could not be entered.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum XrError {
    /// The browser has no `WebXR`, or no immersive VR support.
    #[error("immersive VR is not available in this browser")]
    Unsupported,
    /// A session is already starting or presenting.
    #[error("an XR session is already starting or running")]
    Busy,
    /// The browser refused or failed the session request.
    #[error("the XR session request failed: {0}")]
    Request(String),
}

/// The live session and the closures its JavaScript callbacks point at.
struct Active {
    bridge: XrBridge,
    _on_frame: Closure<dyn FnMut(f64)>,
    _on_end: Closure<dyn FnMut()>,
}

thread_local! {
    static ACTIVE: RefCell<Option<Active>> = const { RefCell::new(None) };
}

/// Whether the browser can present an immersive VR session.
pub async fn is_supported() -> bool {
    bridge::xr_supported().await.as_bool().unwrap_or(false)
}

fn describe(error: &JsValue) -> String {
    error
        .dyn_ref::<js_sys::Error>()
        .map(|e| String::from(e.message()))
        .or_else(|| error.as_string())
        .unwrap_or_else(|| format!("{error:?}"))
}

fn matrix(values: &[f32]) -> Option<Mat4> {
    let array: [f32; 16] = values.try_into().ok()?;
    Some(Mat4::from_cols_array(&array))
}

/// Reads this frame's poses and controllers from the bridge. When the head is not tracked the frame
/// has no views, so the camera keeps its last eyes; the controllers are still read, and a
/// controller that lost tracking reports no poses, which releases whatever it was holding.
fn read_frame(bridge: &XrBridge, time_ms: f64) -> Option<XrFrame> {
    let views = if bridge.pose_valid() {
        (0..bridge.view_count())
            .map(|index| {
                Some(XrView {
                    world_from_view: matrix(&bridge.view_world_from_view(index))?,
                    projection: bridge.view_projection(index).try_into().ok()?,
                    viewport: bridge.view_viewport(index).try_into().ok()?,
                })
            })
            .collect::<Option<Vec<_>>>()?
    } else {
        Vec::new()
    };
    let size = bridge.framebuffer_size();
    let (width, height) = (
        u32::try_from(*size.first()?).ok()?,
        u32::try_from(*size.get(1)?).ok()?,
    );
    Some(XrFrame {
        time_ms,
        views,
        framebuffer_size: (width, height),
        controllers: parse_controllers(&bridge.controllers()),
    })
}

/// One XR frame: publish the poses for the app to apply, then run an app frame.
fn on_frame(bridge: &XrBridge, time_ms: f64) {
    // The wanted view resolution is applied to the next frame's views.
    bridge.set_viewport_scale(f64::from(viewport_scale()));
    if let Some(frame) = read_frame(bridge, time_ms) {
        publish_frame(frame);
    }
    // The app keeps updating even when tracking drops for a frame; `false` means it has exited.
    let _app_is_running = ve_platform_web::pump_frame();
}

fn on_session_end() {
    ve_platform_web::set_externally_driven(false);
    set_session_state(SessionState::Ended);
}

/// Requests an immersive session on the canvas' WebGL context and starts driving the app from the
/// headset's frame loop. Browsers require this to be called from a user gesture on real devices.
/// `resolution_scale` multiplies the runtime's recommended render resolution.
///
/// # Errors
/// [`XrError`] if `WebXR` is unavailable, a session is already running, or the request fails.
pub async fn enter(canvas: &HtmlCanvasElement, resolution_scale: f32) -> Result<(), XrError> {
    if matches!(
        session_state(),
        SessionState::Presenting | SessionState::Requesting
    ) {
        return Err(XrError::Busy);
    }
    // Claim the request before anything is awaited, so a second call cannot slip in meanwhile.
    set_session_state(SessionState::Requesting);
    if !is_supported().await {
        set_last_error("immersive VR is not available in this browser".to_owned());
        set_session_state(SessionState::Failed);
        return Err(XrError::Unsupported);
    }
    let bridge = match XrBridge::request(canvas, f64::from(resolution_scale)).await {
        Ok(bridge) => bridge,
        Err(error) => {
            let message = describe(&error);
            set_last_error(message.clone());
            set_session_state(SessionState::Failed);
            return Err(XrError::Request(message));
        }
    };
    set_refresh_hz(bridge.frame_rate());
    set_viewport_scale_supported(bridge.viewport_scale_supported());
    let for_frame = bridge.clone();
    let frame_closure = Closure::<dyn FnMut(f64)>::new(move |time| on_frame(&for_frame, time));
    let end_closure = Closure::<dyn FnMut()>::new(on_session_end);
    bridge.on_end(&end_closure);
    ve_platform_web::set_externally_driven(true);
    set_session_state(SessionState::Presenting);
    bridge.start(&frame_closure);
    ACTIVE.with(|slot| {
        *slot.borrow_mut() = Some(Active {
            bridge,
            _on_frame: frame_closure,
            _on_end: end_closure,
        });
    });
    Ok(())
}

/// The session layer's framebuffer and its size, when the runtime provides one (real headsets do;
/// emulators draw to the canvas instead).
pub(crate) fn layer_framebuffer() -> Option<(web_sys::WebGlFramebuffer, u32, u32)> {
    ACTIVE.with(|slot| {
        let active = slot.borrow();
        let bridge = &active.as_ref()?.bridge;
        let framebuffer = bridge.framebuffer()?;
        let size = bridge.framebuffer_size();
        let width = u32::try_from(*size.first()?).ok()?;
        let height = u32::try_from(*size.get(1)?).ok()?;
        Some((framebuffer, width, height))
    })
}

/// Ends the running session, if any, and waits for the browser to confirm.
pub async fn exit() {
    let bridge = ACTIVE.with(|slot| slot.borrow().as_ref().map(|active| active.bridge.clone()));
    if let Some(bridge) = bridge {
        // The session may already be gone (headset removed); the end handler has run either way.
        let _ = bridge.end().await;
    }
}
