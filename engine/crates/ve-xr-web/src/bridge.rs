//! The imported JavaScript bridge (`js/xr.js`): plain values in, plain values out.

use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, WebGlFramebuffer};

#[wasm_bindgen(module = "/js/xr.js")]
extern "C" {
    /// Whether an immersive VR session can be requested in this browser.
    #[wasm_bindgen(js_name = xrSupported)]
    pub async fn xr_supported() -> JsValue;

    /// A live `WebXR` session with its layer and reference space.
    #[derive(Clone)]
    pub type XrBridge;

    #[wasm_bindgen(static_method_of = XrBridge, catch)]
    pub async fn request(canvas: &HtmlCanvasElement, scale: f64) -> Result<XrBridge, JsValue>;

    #[wasm_bindgen(method)]
    pub fn start(this: &XrBridge, callback: &Closure<dyn FnMut(f64)>);

    #[wasm_bindgen(method, js_name = onEnd)]
    pub fn on_end(this: &XrBridge, callback: &Closure<dyn FnMut()>);

    #[wasm_bindgen(method, js_name = isEnded)]
    pub fn is_ended(this: &XrBridge) -> bool;

    #[wasm_bindgen(method, catch)]
    pub async fn end(this: &XrBridge) -> Result<(), JsValue>;

    #[wasm_bindgen(method, js_name = poseValid)]
    pub fn pose_valid(this: &XrBridge) -> bool;

    #[wasm_bindgen(method, js_name = viewCount)]
    pub fn view_count(this: &XrBridge) -> u32;

    #[wasm_bindgen(method, js_name = viewWorldFromView)]
    pub fn view_world_from_view(this: &XrBridge, index: u32) -> Vec<f32>;

    #[wasm_bindgen(method, js_name = viewProjection)]
    pub fn view_projection(this: &XrBridge, index: u32) -> Vec<f32>;

    #[wasm_bindgen(method, js_name = viewViewport)]
    pub fn view_viewport(this: &XrBridge, index: u32) -> Vec<i32>;

    #[wasm_bindgen(method, js_name = setViewportScale)]
    pub fn set_viewport_scale(this: &XrBridge, scale: f64);

    #[wasm_bindgen(method, js_name = viewportScaleSupported)]
    pub fn viewport_scale_supported(this: &XrBridge) -> bool;

    #[wasm_bindgen(method, js_name = frameRate)]
    pub fn frame_rate(this: &XrBridge) -> f32;

    #[wasm_bindgen(method, js_name = framebufferSize)]
    pub fn framebuffer_size(this: &XrBridge) -> Vec<i32>;

    #[wasm_bindgen(method)]
    pub fn framebuffer(this: &XrBridge) -> Option<WebGlFramebuffer>;

    #[wasm_bindgen(method)]
    pub fn controllers(this: &XrBridge) -> Vec<f32>;
}
