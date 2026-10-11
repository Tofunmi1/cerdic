//! The `requestAnimationFrame` loop that drives the Bevy `App` in the browser.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use bevy_app::{App, AppExit, PluginsState};
use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_math::Vec2;
use bevy_window::{PrimaryWindow, Window};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;

use crate::canvas::CanvasHandle;
use crate::input::{self, Listeners, Queue};

type FrameCallback = Rc<RefCell<Option<Closure<dyn FnMut()>>>>;

thread_local! {
    /// When set, `requestAnimationFrame` no longer runs frames: an external source (the XR session
    /// loop) calls [`pump_frame`] instead, once per headset frame.
    static EXTERNALLY_DRIVEN: Cell<bool> = const { Cell::new(false) };
    /// The running app, so an external frame source can step it.
    static ACTIVE_RUNNER: RefCell<Option<Rc<RefCell<Runner>>>> = const { RefCell::new(None) };
}

/// Hands frame pacing to an external source (`true`) or back to `requestAnimationFrame` (`false`).
///
/// `WebXR` sessions have their own frame loop; while presenting, the window loop must not also run
/// frames or the app would update twice per display refresh.
pub fn set_externally_driven(driven: bool) {
    EXTERNALLY_DRIVEN.with(|flag| flag.set(driven));
}

/// Runs one app frame now. Returns `false` if no app is running or it has asked to exit.
#[must_use]
pub fn pump_frame() -> bool {
    ACTIVE_RUNNER.with(|slot| {
        let runner = slot.borrow().clone();
        runner.is_some_and(|runner| runner.borrow_mut().frame())
    })
}

struct Runner {
    app: App,
    canvas: CanvasHandle,
    queue: Queue,
    _listeners: Listeners,
    plugins_ready: bool,
    last_cursor: Option<Vec2>,
    last_size: (u32, u32),
}

/// Entry point installed with `App::set_runner`. Returns immediately: the browser owns the loop.
pub(crate) fn run(app: App, canvas: CanvasHandle, queue: Queue, listeners: Listeners) -> AppExit {
    let runner = Runner {
        app,
        canvas,
        queue,
        _listeners: listeners,
        plugins_ready: false,
        last_cursor: None,
        last_size: (0, 0),
    };
    start(Rc::new(RefCell::new(runner)));
    AppExit::Success
}

fn start(runner: Rc<RefCell<Runner>>) {
    ACTIVE_RUNNER.with(|slot| *slot.borrow_mut() = Some(Rc::clone(&runner)));
    let slot: FrameCallback = Rc::new(RefCell::new(None));
    let next = Rc::clone(&slot);
    *slot.borrow_mut() = Some(Closure::new(move || {
        let keep_going = EXTERNALLY_DRIVEN.with(Cell::get) || runner.borrow_mut().frame();
        if keep_going {
            request_frame(&next);
        } else {
            // Break the Rc cycles so the runner and its listeners are released.
            next.borrow_mut().take();
            ACTIVE_RUNNER.with(|slot| slot.borrow_mut().take());
        }
    }));
    request_frame(&slot);
}

fn request_frame(slot: &FrameCallback) {
    if let (Some(window), Some(callback)) = (web_sys::window(), slot.borrow().as_ref()) {
        // Only fails if the window is closing; there is nothing useful to do then.
        let _ = window.request_animation_frame(callback.as_ref().unchecked_ref());
    }
}

impl Runner {
    /// Runs one frame. Returns `false` once the app has asked to exit.
    fn frame(&mut self) -> bool {
        if !self.plugins_ready && !self.try_finish_plugins() {
            return true;
        }
        let Some(window) = self.primary_window() else {
            return self.app.should_exit().is_none();
        };
        self.sync_size(window);
        input::drain(
            self.app.world_mut(),
            window,
            &self.queue,
            &mut self.last_cursor,
        );
        self.app.update();
        self.app.should_exit().is_none()
    }

    /// Plugins such as the renderer finish asynchronously; wait for them before updating.
    fn try_finish_plugins(&mut self) -> bool {
        match self.app.plugins_state() {
            PluginsState::Adding => return false,
            PluginsState::Ready => {
                self.app.finish();
                self.app.cleanup();
            }
            PluginsState::Finished => self.app.cleanup(),
            PluginsState::Cleaned => {}
        }
        self.plugins_ready = true;
        true
    }

    fn primary_window(&mut self) -> Option<Entity> {
        let world = self.app.world_mut();
        world
            .query_filtered::<Entity, With<PrimaryWindow>>()
            .single(world)
            .ok()
    }

    /// Keeps the drawing buffer and the Bevy `Window` in step with the canvas' CSS size.
    fn sync_size(&mut self, window: Entity) {
        let (width, height, ratio) = self.canvas.physical_size();
        if (width, height) == self.last_size {
            return;
        }
        self.last_size = (width, height);
        self.canvas.set_buffer_size(width, height);
        if let Some(mut component) = self.app.world_mut().get_mut::<Window>(window) {
            component.resolution.set_physical_resolution(width, height);
            #[expect(
                clippy::cast_possible_truncation,
                reason = "device pixel ratios fit f32"
            )]
            component.resolution.set_scale_factor(ratio as f32);
        }
    }
}
