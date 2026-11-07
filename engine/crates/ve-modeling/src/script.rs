//! Runs one user script (JavaScript, QuickJS) under hard limits, and gives the same answer on every run.
//!
//! The limits are work (interrupt polls, about ten thousand operations each), memory and stack. The page can also stop a script
//! through `cancel`. Nothing reaches outside the engine: the script sees no files, network, clock or console. `Math.random` is
//! replaced by a seeded generator and `Date` is removed, so a script is a pure function of its source, its seed and its inputs.
use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

use rquickjs::{Context, Ctx, Error, Exception, Object, Runtime, Value};

/// What a script may use before it is stopped.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Interrupt polls (about ten thousand operations each).
    pub polls: u64,
    /// Bytes of script memory.
    pub memory: usize,
    /// Bytes of call stack.
    pub stack: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            polls: 2_000,
            memory: 64 * 1024 * 1024,
            stack: 128 * 1024,
        }
    }
}

/// Why a script did not produce a value.
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptError {
    /// What went wrong, in the script engine's words.
    pub message: String,
    /// One-based line in the script, when the engine knows it.
    pub line: Option<u32>,
    /// One-based column in the script, when the engine knows it.
    pub column: Option<u32>,
    /// True when the script was stopped for using its work budget or by cancellation.
    pub stopped: bool,
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.line, self.column) {
            (Some(line), Some(column)) => {
                write!(f, "{} (line {line}, column {column})", self.message)
            }
            (Some(line), None) => write!(f, "{} (line {line})", self.message),
            _ => f.write_str(&self.message),
        }
    }
}

/// The seeded `Math.random` and the removal of the clock. Runs before the script, in its own evaluation, so script line
/// numbers start at 1.
fn prelude(seed: u32) -> String {
    format!(
        "(function () {{ let s = {} >>> 0 || 1; Math.random = function () {{ s ^= s << 13; s >>>= 0; s ^= s >>> 17; s ^= s << 5; s >>>= 0; return s / 4294967296; }}; delete globalThis.Date; }})();",
        seed
    )
}

/// Runs `source` and returns its final value as text (objects and arrays as JSON).
///
/// # Errors
/// A [`ScriptError`] when the script throws, does not parse, runs out of work, memory or stack, or is cancelled.
pub fn run(
    source: &str,
    seed: u32,
    limits: &Limits,
    cancel: Rc<dyn Fn() -> bool>,
) -> Result<String, ScriptError> {
    run_with(source, seed, limits, cancel, &[], None)
}

/// QuickJS's wasm shim reads the time through a host import (`env.__rquickjs_host_now_us`) when it seeds its generator or builds a `Date`.
/// Defining the function here answers that import inside the module (the link resolves the shim's call to this) so the page has nothing to
/// provide, and the answer is always zero: a script can never see a real clock, which keeps its results a function of its source and seed.
#[cfg(target_arch = "wasm32")]
#[expect(
    unsafe_code,
    reason = "an exported C symbol the QuickJS wasm shim links against has to be unmangled"
)]
#[unsafe(no_mangle)]
extern "C" fn __rquickjs_host_now_us() -> f64 {
    0.0
}

/// A function the host offers the script as the global `__host(a, b, c)`: three texts in, one text out, or a message the script
/// sees as a thrown error.
pub type Host = Rc<dyn Fn(&str, &str, &str) -> Result<String, String>>;

/// Like [`run`], with `setup` evaluated first, each in its own evaluation so the script's line numbers still start at 1: the
/// host's helper functions and the values the script may read (a global named for each input, for example).
///
/// # Errors
/// A [`ScriptError`] when the script throws, does not parse, runs out of work, memory or stack, or is cancelled. A failing
/// `setup` snippet is reported as the engine not starting.
pub fn run_with(
    source: &str,
    seed: u32,
    limits: &Limits,
    cancel: Rc<dyn Fn() -> bool>,
    setup: &[String],
    host: Option<Host>,
) -> Result<String, ScriptError> {
    let plain = |message: &str| ScriptError {
        message: message.to_owned(),
        line: None,
        column: None,
        stopped: false,
    };
    // Our own allocator, not the C library's: the memory cap needs block sizes, which not every target's C library can report.
    let runtime = Runtime::new_with_alloc(rquickjs::allocator::RustAllocator)
        .map_err(|_| plain("the script engine did not start"))?;
    runtime.set_memory_limit(limits.memory);
    runtime.set_max_stack_size(limits.stack);
    let polls = Rc::new(Cell::new(0_u64));
    let stopped = Rc::new(Cell::new(false));
    {
        let (polls, stopped) = (Rc::clone(&polls), Rc::clone(&stopped));
        let budget = limits.polls;
        runtime.set_interrupt_handler(Some(Box::new(move || {
            polls.set(polls.get() + 1);
            if polls.get() > budget || cancel() {
                stopped.set(true);
            }
            stopped.get()
        })));
    }
    let context = Context::full(&runtime).map_err(|_| plain("the script engine did not start"))?;
    context.with(|ctx| {
        ctx.eval::<(), _>(prelude(seed))
            .map_err(|_| plain("the script engine did not start"))?;
        if let Some(host) = host {
            let function = rquickjs::Function::new(
                ctx.clone(),
                move |ctx: Ctx<'_>, a: String, b: String, c: String| -> rquickjs::Result<String> {
                    host(&a, &b, &c).map_err(|message| Exception::throw_message(&ctx, &message))
                },
            )
            .map_err(|_| plain("the script engine did not start"))?;
            ctx.globals()
                .set("__host", function)
                .map_err(|_| plain("the script engine did not start"))?;
        }
        for snippet in setup {
            ctx.eval::<(), _>(snippet.as_str())
                .map_err(|_| plain("the script's helpers did not load"))?;
        }
        match ctx.eval::<Value, _>(source) {
            Ok(value) => Ok(show(&ctx, &value)),
            Err(Error::Exception) => {
                let was_stopped = stopped.get();
                let mut error = describe(&ctx.catch());
                if was_stopped {
                    error.message =
                        "the script was stopped: it used up its work budget or was cancelled"
                            .to_owned();
                    error.stopped = true;
                }
                Err(error)
            }
            Err(other) => Err(plain(&other.to_string())),
        }
    })
}

/// A value as text: strings as they are, numbers and others the way JavaScript prints them, objects as JSON.
fn show<'a>(ctx: &Ctx<'a>, value: &Value<'a>) -> String {
    if let Some(string) = value.as_string() {
        return string.to_string().unwrap_or_default();
    }
    if value.is_object() {
        if let Ok(Some(json)) = ctx.json_stringify(value.clone()) {
            if let Ok(text) = json.to_string() {
                return text;
            }
        }
    }
    value
        .get::<rquickjs::Coerced<String>>()
        .map(|text| text.0)
        .unwrap_or_default()
}

/// The thrown value's message and where in the script it was thrown.
fn describe(thrown: &Value<'_>) -> ScriptError {
    let object: Option<&Object<'_>> = thrown.as_object();
    let message = object
        .and_then(|o| {
            o.get::<_, Option<rquickjs::Coerced<String>>>("message")
                .ok()
                .flatten()
        })
        .map(|text| text.0)
        .or_else(|| {
            thrown
                .get::<rquickjs::Coerced<String>>()
                .ok()
                .map(|text| text.0)
        })
        .unwrap_or_else(|| "the script failed".to_owned());
    let name = object
        .and_then(|o| {
            o.get::<_, Option<rquickjs::Coerced<String>>>("name")
                .ok()
                .flatten()
        })
        .map(|text| text.0);
    let stack = object
        .and_then(|o| {
            o.get::<_, Option<rquickjs::Coerced<String>>>("stack")
                .ok()
                .flatten()
        })
        .map(|text| text.0)
        .unwrap_or_default();
    let (line, column) = position(&stack);
    ScriptError {
        message: match name {
            Some(name) if name != "Error" && !message.starts_with(&name) => {
                format!("{name}: {message}")
            }
            _ => message,
        },
        line,
        column,
        stopped: false,
    }
}

/// The first `eval_script:line:column` in an engine stack trace.
fn position(stack: &str) -> (Option<u32>, Option<u32>) {
    let Some(at) = stack.find("eval_script:") else {
        return (None, None);
    };
    let mut parts = stack[at + "eval_script:".len()..]
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty());
    let line = parts.next().and_then(|n| n.parse().ok());
    let column = parts.next().and_then(|n| n.parse().ok());
    (line, column)
}
