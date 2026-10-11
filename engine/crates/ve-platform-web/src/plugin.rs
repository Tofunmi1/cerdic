//! The plugin that makes a canvas the primary window and installs the browser runner.

use std::sync::{Arc, Mutex};

use bevy_app::{App, Plugin};
use bevy_ecs::resource::Resource;
use bevy_window::{
    PrimaryWindow, RawHandleWrapper, RawHandleWrapperHolder, Window, WindowResolution,
    WindowWrapper,
};
use wasm_bindgen::JsCast;
use web_sys::EventTarget;

use crate::PlatformError;
use crate::canvas::CanvasHandle;
use crate::input::{Listeners, Queue};
use crate::runner;

/// Reported when the canvas handle could not be turned into a raw window handle.
#[derive(Clone, Debug, Resource)]
pub struct PlatformStartupError(pub String);

/// Uses a `<canvas>` as the primary window and drives the app from `requestAnimationFrame`.
///
/// Add it before the render plugin: the renderer reads the window handle while it builds.
#[derive(Clone, Debug)]
pub struct WebPlatformPlugin {
    canvas: CanvasHandle,
}

impl WebPlatformPlugin {
    /// Looks up the canvas element with the given id.
    ///
    /// # Errors
    /// [`PlatformError`] if the canvas cannot be found.
    pub fn new(canvas_id: &str) -> Result<Self, PlatformError> {
        Ok(Self {
            canvas: CanvasHandle::find(canvas_id)?,
        })
    }
}

impl Plugin for WebPlatformPlugin {
    fn build(&self, app: &mut App) {
        let (width, height, ratio) = self.canvas.physical_size();
        self.canvas.set_buffer_size(width, height);

        let mut resolution = WindowResolution::new(width, height);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "device pixel ratios fit f32"
        )]
        resolution.set_scale_factor(ratio as f32);
        let window = Window {
            resolution,
            ..Window::default()
        };

        let wrapper = WindowWrapper::new(self.canvas.clone());
        let handle = RawHandleWrapper::new(&wrapper);
        let mut entity = app.world_mut().spawn((window, PrimaryWindow));
        match handle {
            Ok(raw) => {
                entity.insert((
                    raw.clone(),
                    RawHandleWrapperHolder(Arc::new(Mutex::new(Some(raw)))),
                ));
            }
            Err(error) => {
                let message = format!("canvas window handle unavailable: {error}");
                app.insert_resource(PlatformStartupError(message));
            }
        }

        let queue = Queue::default();
        let target: &EventTarget = self.canvas.element().unchecked_ref();
        let listeners = Listeners::attach(target, &queue);
        let canvas = self.canvas.clone();
        app.set_runner(move |app| runner::run(app, canvas, queue, listeners));
    }
}
