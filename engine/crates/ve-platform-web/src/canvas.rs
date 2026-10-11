//! The canvas as a raw-window-handle source, plus size helpers.

use core::ffi::c_void;
use core::ptr::NonNull;

use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawWindowHandle,
    WebCanvasWindowHandle, WindowHandle,
};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::HtmlCanvasElement;

use crate::PlatformError;

/// A canvas element that wgpu can create a surface on.
#[derive(Clone, Debug)]
pub struct CanvasHandle {
    canvas: HtmlCanvasElement,
}

// SAFETY: this crate refuses to compile with the `atomics` target feature (see lib.rs), so there is
// exactly one thread and the JS object is never touched from another one.
unsafe impl Send for CanvasHandle {}
// SAFETY: same single-thread guarantee as above.
unsafe impl Sync for CanvasHandle {}

impl CanvasHandle {
    /// Finds the canvas with the given element id.
    ///
    /// # Errors
    /// [`PlatformError`] if there is no document, no such element, or it is not a canvas.
    pub fn find(id: &str) -> Result<Self, PlatformError> {
        let document = web_sys::window()
            .and_then(|w| w.document())
            .ok_or(PlatformError::NoWindow)?;
        let element = document
            .get_element_by_id(id)
            .ok_or_else(|| PlatformError::CanvasNotFound(id.to_owned()))?;
        let canvas = element
            .dyn_into::<HtmlCanvasElement>()
            .map_err(|_| PlatformError::NotACanvas(id.to_owned()))?;
        Ok(Self { canvas })
    }

    /// The underlying element.
    #[must_use]
    pub fn element(&self) -> &HtmlCanvasElement {
        &self.canvas
    }

    /// Current drawing-buffer size in physical pixels and the device pixel ratio, derived from the
    /// element's CSS size. Never returns a zero dimension.
    #[must_use]
    pub fn physical_size(&self) -> (u32, u32, f64) {
        let ratio = web_sys::window()
            .map_or(1.0, |w| w.device_pixel_ratio())
            .max(1.0);
        let to_px = |css: i32| -> u32 {
            let scaled = (f64::from(css.max(1)) * ratio).round();
            // Clamped to a sane range first, so the conversion cannot truncate or go negative.
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "value clamped to 1..=16384"
            )]
            let px = scaled.clamp(1.0, 16384.0) as u32;
            px
        };
        (
            to_px(self.canvas.client_width()),
            to_px(self.canvas.client_height()),
            ratio,
        )
    }

    /// Resizes the canvas drawing buffer to `width` x `height` physical pixels.
    pub fn set_buffer_size(&self, width: u32, height: u32) {
        self.canvas.set_width(width);
        self.canvas.set_height(height);
    }
}

impl HasWindowHandle for CanvasHandle {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let value: &JsValue = &self.canvas;
        let object = NonNull::from(value).cast::<c_void>();
        let raw = RawWindowHandle::WebCanvas(WebCanvasWindowHandle::new(object));
        // SAFETY: `object` points at the `JsValue` inside `self.canvas`, which outlives the borrow of
        // `self` that bounds the returned handle; wgpu reads it as a `&JsValue`.
        Ok(unsafe { WindowHandle::borrow_raw(raw) })
    }
}

impl HasDisplayHandle for CanvasHandle {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::web())
    }
}
