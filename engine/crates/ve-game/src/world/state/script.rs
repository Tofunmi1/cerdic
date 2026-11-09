//! The script host: JavaScript behaviours that run inside the fixed step.
//!
//! A game declares `script` statements; each is evaluated once when the world is created and registers handlers with `on(event, fn)`,
//! `every(seconds, fn)` and `after(seconds, fn)`. Every event the world raises (the ones rules hear, and the ones scripts and rules emit)
//! is then offered to the handlers, inside the step, in the order it happens. The VM is QuickJS inside our own build, so it behaves the
//! same on every machine, has no clock (`Date` is gone), draws `Math.random` from the world's seeded stream, and is stopped when it uses
//! up its work budget. What a script wants to keep across a save, an undo or a replay lives in the global `state` (plain JSON): that is
//! what a snapshot stores and a restore puts back, and the script's own source re-registers its handlers.
//!
//! QuickJS values cannot cross threads, so the VMs live in a thread-local table and a world holds only a handle to its own.
mod bridge;

use super::{Event, World};
use crate::{Effect, GameDef, GameError};
use bridge::Bridge;
use rquickjs::{Context, Ctx, Error, Exception, Function, Runtime};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

const MEMORY_BYTES: usize = 64 << 20;
const STACK_BYTES: usize = 512 << 10;
/// Work allowed for one dispatch, in interrupt polls (QuickJS polls about every ten thousand operations).
const POLLS_PER_CALL: u64 = 2000;
const PRELUDE: &str = include_str!("script/prelude.js");

struct Host {
    _runtime: Runtime,
    context: Context,
    bridge: Rc<RefCell<Bridge>>,
    polls: Rc<Cell<u64>>,
    stopped: Rc<Cell<bool>>,
}

thread_local! {
    static HOSTS: RefCell<BTreeMap<u64, Host>> = const { RefCell::new(BTreeMap::new()) };
    static NEXT_HANDLE: Cell<u64> = const { Cell::new(1) };
}

/// A world's claim on its VM; dropping the world drops the VM.
pub(super) struct Handle(u64);

impl Drop for Handle {
    fn drop(&mut self) {
        let _ = HOSTS.try_with(|hosts| hosts.borrow_mut().remove(&self.0));
    }
}

fn thrown(ctx: &Ctx<'_>) -> String {
    let value = ctx.catch();
    value
        .as_object()
        .and_then(|object| {
            object
                .get::<_, Option<rquickjs::Coerced<String>>>("message")
                .ok()
                .flatten()
        })
        .map(|text| text.0)
        .or_else(|| {
            value
                .get::<rquickjs::Coerced<String>>()
                .ok()
                .map(|text| text.0)
        })
        .unwrap_or_else(|| "the script failed".to_owned())
}

impl Host {
    fn new(def: &GameDef, seed: u64) -> Result<Self, String> {
        let engine = "the script engine did not start";
        let runtime =
            Runtime::new_with_alloc(rquickjs::allocator::RustAllocator).map_err(|_| engine)?;
        runtime.set_memory_limit(MEMORY_BYTES);
        runtime.set_max_stack_size(STACK_BYTES);
        let polls = Rc::new(Cell::new(0_u64));
        let stopped = Rc::new(Cell::new(false));
        {
            let (polls, stopped) = (Rc::clone(&polls), Rc::clone(&stopped));
            runtime.set_interrupt_handler(Some(Box::new(move || {
                polls.set(polls.get() + 1);
                if polls.get() > POLLS_PER_CALL {
                    stopped.set(true);
                }
                stopped.get()
            })));
        }
        let context = Context::full(&runtime).map_err(|_| engine)?;
        let bridge = Rc::new(RefCell::new(Bridge {
            def: def.clone(),
            entities: Vec::new(),
            vars: BTreeMap::new(),
            rng: seed,
            next_id: 0,
            status: crate::Status::Playing,
            raised: Vec::new(),
            effects: Vec::new(),
            destroyed: Vec::new(),
            spawned: Vec::new(),
            changed: Vec::new(),
            log: Vec::new(),
            ui: Vec::new(),
            axes: BTreeMap::new(),
            presses: Vec::new(),
            camera: None,
            reset: false,
        }));
        let host = Self {
            _runtime: runtime,
            context,
            bridge,
            polls,
            stopped,
        };
        host.context.with(|ctx| -> Result<(), String> {
            let shared = Rc::clone(&host.bridge);
            let function = Function::new(
                ctx.clone(),
                move |ctx: Ctx<'_>, op: String, args: String| -> rquickjs::Result<String> {
                    let args = serde_json::from_str(&args).map_err(|error| {
                        Exception::throw_message(&ctx, &format!("bad arguments: {error}"))
                    })?;
                    let result = shared.borrow_mut().call(&op, &args);
                    result
                        .map(|value| value.to_string())
                        .map_err(|message| Exception::throw_message(&ctx, &message))
                },
            )
            .map_err(|_| engine)?;
            ctx.globals().set("__host", function).map_err(|_| engine)?;
            ctx.eval::<(), _>(PRELUDE).map_err(|_| engine.to_owned())
        })?;
        Ok(host)
    }

    fn reset_budget(&self) {
        self.polls.set(0);
        self.stopped.set(false);
    }

    /// Evaluates one declared script inside its own function scope (line numbers stay the script's own).
    fn load(&self, name: &str, source: &str) {
        self.reset_budget();
        let wrapped = format!("__load({name:?},function(){{'use strict';{source}\n}})");
        let outcome = self.context.with(|ctx| match ctx.eval::<(), _>(wrapped) {
            Ok(()) => None,
            Err(Error::Exception) => Some(thrown(&ctx)),
            Err(other) => Some(other.to_string()),
        });
        if let Some(message) = outcome {
            let message = if self.stopped.get() {
                "it used up its work budget".to_owned()
            } else {
                message
            };
            self.bridge
                .borrow_mut()
                .log
                .push(format!("error: script '{name}' did not start: {message}"));
        }
    }

    fn dispatch(
        &self,
        name: &str,
        a: Option<u32>,
        b: Option<u32>,
        tick: u64,
    ) -> Result<(), String> {
        self.reset_budget();
        let id = |value: Option<u32>| value.map_or(-1_i64, i64::from);
        self.context.with(|ctx| {
            let function: Function<'_> = ctx
                .globals()
                .get("__dispatch")
                .map_err(|error| error.to_string())?;
            match function.call::<_, ()>((name, id(a), id(b), tick)) {
                Ok(()) => Ok(()),
                Err(Error::Exception) if self.stopped.get() => Err(format!(
                    "a script used up its work budget while handling '{name}'"
                )),
                Err(Error::Exception) => Err(thrown(&ctx)),
                Err(other) => Err(other.to_string()),
            }
        })
    }

    fn state(&self) -> String {
        self.reset_budget();
        self.context.with(|ctx| {
            ctx.globals()
                .get::<_, Function<'_>>("__state")
                .and_then(|function| function.call::<_, String>(()))
                .unwrap_or_else(|_| "{}".to_owned())
        })
    }

    fn restore(&self, text: &str) {
        self.reset_budget();
        self.context.with(|ctx| {
            let _ = ctx
                .globals()
                .get::<_, Function<'_>>("__restore")
                .and_then(|function| function.call::<_, ()>((text,)));
        });
    }
}

fn with_host<R>(handle: &Handle, f: impl FnOnce(&Host) -> R) -> Option<R> {
    HOSTS.with(|hosts| hosts.borrow().get(&handle.0).map(f))
}

impl World {
    /// Starts the script host when the game declares scripts and evaluates each one. A script that fails to start is reported in the
    /// script log and skipped.
    pub(super) fn start_scripts(&mut self, seed: u64) {
        if self.def.scripts.is_empty() {
            return;
        }
        let handle = NEXT_HANDLE.with(|next| {
            let value = next.get();
            next.set(value + 1);
            value
        });
        match Host::new(&self.def, seed) {
            Ok(host) => {
                for script in &self.def.scripts {
                    host.load(&script.name, &script.source);
                }
                HOSTS.with(|hosts| hosts.borrow_mut().insert(handle, host));
                self.script = Some(Handle(handle));
            }
            Err(message) => self.script_failure = Some(format!("error: {message}")),
        }
    }

    /// Offers one event to the scripts, inside the step. Whatever they changed comes back into the world and whatever they raised joins
    /// the queue. Returns how many actions that counted as.
    pub(super) fn script_event(
        &mut self,
        event: &Event,
        queue: &mut Vec<Event>,
        effects: &mut Vec<Effect>,
    ) -> Result<usize, GameError> {
        let Some(handle) = &self.script else {
            return Ok(0);
        };
        let tick = self.tick;
        let outcome = with_host(handle, |host| {
            {
                let mut bridge = host.bridge.borrow_mut();
                bridge.entities = std::mem::take(&mut self.entities);
                bridge.vars = std::mem::take(&mut self.vars);
                bridge.rng = self.rng;
                bridge.next_id = self.next_id;
                bridge.status = self.status;
            }
            let result = host.dispatch(&event.name, event.a, event.b, tick);
            let mut bridge = host.bridge.borrow_mut();
            self.entities = std::mem::take(&mut bridge.entities);
            self.vars = std::mem::take(&mut bridge.vars);
            self.rng = bridge.rng;
            self.next_id = bridge.next_id;
            self.status = bridge.status;
            let spawned = std::mem::take(&mut bridge.spawned);
            let destroyed = std::mem::take(&mut bridge.destroyed);
            let changed = std::mem::take(&mut bridge.changed);
            let raised = std::mem::take(&mut bridge.raised);
            effects.append(&mut bridge.effects);
            (result, spawned, destroyed, changed, raised)
        });
        let Some((result, spawned, destroyed, changed, raised)) = outcome else {
            return Err(GameError::Script(
                "the world's scripts belong to another thread".into(),
            ));
        };
        result.map_err(GameError::Script)?;
        self.entities.sort_by_key(|entity| entity.id);
        for id in spawned {
            if let Some(entity) = self.entities.iter().find(|entity| entity.id == id)
                && self
                    .def
                    .agents
                    .iter()
                    .any(|agent| agent.kind == entity.kind)
            {
                self.agents.insert(id, super::agents::AgentState::default());
            }
        }
        let depth = event.depth + 1;
        for id in destroyed {
            self.agents.remove(&id);
            queue.push(Event {
                name: "destroyed".into(),
                a: Some(id),
                b: None,
                depth,
            });
        }
        for name in changed {
            queue.push(Event {
                name: format!("var:{name}"),
                a: event.a,
                b: event.b,
                depth,
            });
        }
        for (name, a, b) in raised {
            queue.push(Event {
                name: format!("emit:{name}"),
                a,
                b,
                depth,
            });
        }
        Ok(1)
    }

    /// The lines scripts logged (and the errors they raised) since the last call, oldest first.
    pub fn take_script_log(&mut self) -> Vec<String> {
        let mut lines: Vec<String> = self.script_failure.take().into_iter().collect();
        if let Some(handle) = &self.script {
            lines.extend(
                with_host(handle, |host| {
                    std::mem::take(&mut host.bridge.borrow_mut().log)
                })
                .unwrap_or_default(),
            );
        }
        lines
    }

    /// Hands the step's host input to the scripts (`input.axis`, `input.pressed`); called once at the start of each step.
    pub(super) fn script_begin_step(&mut self, input: &crate::Inputs) {
        if let Some(handle) = &self.script {
            with_host(handle, |host| {
                let mut bridge = host.bridge.borrow_mut();
                bridge.axes.clone_from(&input.axes);
                bridge.presses.clone_from(&input.pressed);
            });
        }
    }

    /// Whether a script asked for the game to start over since the last check (and clears the request).
    pub(super) fn script_take_reset(&self) -> bool {
        self.script
            .as_ref()
            .and_then(|handle| {
                with_host(handle, |host| {
                    std::mem::take(&mut host.bridge.borrow_mut().reset)
                })
            })
            .unwrap_or(false)
    }

    /// Where the scripts want the view this step: the eye position and the point looked at, or `None` when they never set one.
    #[must_use]
    pub fn script_camera(&self) -> Option<([f64; 3], [f64; 3])> {
        self.script
            .as_ref()
            .and_then(|handle| with_host(handle, |host| host.bridge.borrow().camera))
            .flatten()
    }

    /// The game UI the scripts have set, laid out for a screen of this aspect ratio (width over height) as JSON:
    /// `{aspect, items}` with `rect` and `path` items in screen heights, y down. Empty when the game has no scripts.
    #[must_use]
    pub fn ui_frame(&self, aspect: f64) -> String {
        // the declared tree first, then whatever the scripts have set on top of it
        let mut elements = self.declared_ui_specs(aspect);
        if let Some(handle) = &self.script {
            elements.extend(with_host(handle, |host| host.bridge.borrow().ui.clone()).unwrap_or_default());
        }
        let mut frame = crate::ui::frame(&elements, aspect);
        // where the buttons are, so the page can turn a click into the press `ui:ID` without knowing the layout rules
        let buttons: Vec<serde_json::Value> = self
            .ui_layout(aspect)
            .iter()
            .filter(|b| b.kind == "button")
            .map(|b| serde_json::json!({ "id": b.id, "x": b.rect[0], "y": b.rect[1], "w": b.rect[2], "h": b.rect[3] }))
            .collect();
        frame["buttons"] = serde_json::Value::Array(buttons);
        frame.to_string()
    }

    /// The scripts' persistent `state` as JSON, for snapshots and hashes.
    pub(super) fn script_state(&self) -> Option<String> {
        self.script
            .as_ref()
            .and_then(|handle| with_host(handle, Host::state))
    }

    /// Puts a snapshot's script `state` back.
    pub(super) fn script_restore(&self, state: Option<&str>) {
        if let (Some(handle), Some(state)) = (&self.script, state) {
            with_host(handle, |host| host.restore(state));
        }
    }
}
