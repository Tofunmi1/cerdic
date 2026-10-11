//! State shared between the browser's XR frame callback and the Bevy app.
//!
//! The XR callback runs outside the app, so frames cross over through thread-local slots (the
//! engine is single-threaded in the browser). The app reads them in `apply_xr_frame`.

use std::cell::{Cell, RefCell};

use crate::frame::XrFrame;

/// Where the XR session is in its life.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SessionState {
    /// No session was requested.
    #[default]
    Idle,
    /// A session request is in flight (waiting on the browser, and the user's permission).
    Requesting,
    /// The headset is presenting and frames are flowing.
    Presenting,
    /// The session ended normally.
    Ended,
    /// The session could not start; see [`last_error`].
    Failed,
}

thread_local! {
    static LATEST: RefCell<Option<XrFrame>> = const { RefCell::new(None) };
    static LAST_SEEN: RefCell<Option<XrFrame>> = const { RefCell::new(None) };
    static STATE: Cell<SessionState> = const { Cell::new(SessionState::Idle) };
    static FRAMES: Cell<u64> = const { Cell::new(0) };
    static ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
    static REFRESH_HZ: Cell<f32> = const { Cell::new(0.0) };
}

/// Records the display refresh rate the session reports (zero when the runtime does not say).
pub fn set_refresh_hz(hz: f32) {
    REFRESH_HZ.with(|cell| cell.set(if hz.is_finite() && hz > 0.0 { hz } else { 0.0 }));
}

/// The refresh rate of the current or last session, if the runtime reported one.
#[must_use]
pub fn refresh_hz() -> Option<f32> {
    let hz = REFRESH_HZ.with(Cell::get);
    (hz > 0.0).then_some(hz)
}

/// Stores the newest frame for the app to pick up, replacing an unread one.
pub fn publish_frame(frame: XrFrame) {
    FRAMES.with(|frames| frames.set(frames.get() + 1));
    LAST_SEEN.with(|slot| slot.borrow_mut().clone_from(&Some(frame.clone())));
    LATEST.with(|slot| *slot.borrow_mut() = Some(frame));
}

/// A copy of the most recent frame the runtime reported, kept for diagnostics even after the app
/// has consumed it.
#[must_use]
pub fn last_frame() -> Option<XrFrame> {
    LAST_SEEN.with(|slot| slot.borrow().clone())
}

/// Takes the newest unread frame, if any.
#[must_use]
pub fn take_latest() -> Option<XrFrame> {
    LATEST.with(|slot| slot.borrow_mut().take())
}

/// The current session state.
#[must_use]
pub fn session_state() -> SessionState {
    STATE.with(Cell::get)
}

/// Moves the session to a new state.
pub fn set_session_state(state: SessionState) {
    STATE.with(|cell| cell.set(state));
}

/// How many XR frames have been published since the page loaded.
#[must_use]
pub fn frames_published() -> u64 {
    FRAMES.with(Cell::get)
}

/// Records why the session failed.
pub fn set_last_error(message: String) {
    ERROR.with(|slot| *slot.borrow_mut() = Some(message));
}

/// The reason for the most recent failure, if any.
#[must_use]
pub fn last_error() -> Option<String> {
    ERROR.with(|slot| slot.borrow().clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(time_ms: f64) -> XrFrame {
        XrFrame {
            time_ms,
            views: vec![],
            framebuffer_size: (1, 1),
            controllers: vec![],
        }
    }

    #[test]
    fn the_app_sees_only_the_newest_frame_once() {
        assert!(take_latest().is_none());
        publish_frame(frame(1.0));
        publish_frame(frame(2.0));
        let taken = take_latest().unwrap();
        assert!((taken.time_ms - 2.0).abs() < f64::EPSILON);
        assert!(take_latest().is_none());
        assert_eq!(frames_published(), 2);
    }

    #[test]
    fn state_and_error_round_trip() {
        assert_eq!(session_state(), SessionState::Idle);
        set_session_state(SessionState::Failed);
        set_last_error("denied".to_owned());
        assert_eq!(session_state(), SessionState::Failed);
        assert_eq!(last_error().as_deref(), Some("denied"));
    }
}
