//! The script engine: results, repeatability, readable errors with positions, and every limit.
use std::cell::Cell;
use std::rc::Rc;

use ve_modeling::script::{Limits, run};

fn never() -> Rc<dyn Fn() -> bool> {
    Rc::new(|| false)
}

fn go(source: &str) -> Result<String, ve_modeling::script::ScriptError> {
    run(source, 7, &Limits::default(), never())
}

#[test]
fn returns_numbers_strings_and_json() {
    assert_eq!(go("1 + 2").unwrap_or_default(), "3");
    assert_eq!(go("'a' + 'b'").unwrap_or_default(), "ab");
    assert_eq!(go("[1, 2, {a: 3}]").unwrap_or_default(), "[1,2,{\"a\":3}]");
}

#[test]
fn random_is_seeded_and_repeatable() {
    let source = "[Math.random(), Math.random()].join(',')";
    let a = run(source, 5, &Limits::default(), never());
    let b = run(source, 5, &Limits::default(), never());
    let c = run(source, 6, &Limits::default(), never());
    assert_eq!(a, b, "same seed, same numbers");
    assert_ne!(a, c, "a different seed gives different numbers");
    let numbers = a.unwrap_or_default();
    assert!(
        numbers
            .split(',')
            .all(|n| n.parse::<f64>().is_ok_and(|v| (0.0..1.0).contains(&v)))
    );
}

#[test]
fn the_clock_is_gone() {
    assert_eq!(go("typeof Date").unwrap_or_default(), "undefined");
}

#[test]
fn the_outside_world_is_not_reachable() {
    let seen = go("['fetch','require','process','window','document','console','setTimeout'].map(n => typeof globalThis[n]).join()")
        .unwrap_or_default();
    assert_eq!(
        seen,
        "undefined,undefined,undefined,undefined,undefined,undefined,undefined"
    );
}

#[test]
fn a_thrown_error_names_the_message_and_the_line() {
    let error = go("let a = 1;\nlet b = 2;\nthrow new Error('no good');").err();
    let error = error.unwrap_or_else(|| panic_free());
    assert_eq!(error.message, "no good");
    assert_eq!(error.line, Some(3));
    assert!(!error.stopped);
}

#[test]
fn a_type_error_names_its_kind_and_position() {
    let error = go("let a = 1;\nundefinedFunction(a);")
        .err()
        .unwrap_or_else(|| panic_free());
    assert!(error.message.contains("undefinedFunction"), "{error}");
    assert!(error.message.starts_with("ReferenceError"), "{error}");
    assert_eq!(error.line, Some(2));
}

#[test]
fn a_syntax_error_is_reported_not_swallowed() {
    let error = go("let = ;").err().unwrap_or_else(|| panic_free());
    assert!(
        error.message.contains("SyntaxError") || error.message.contains("expecting"),
        "{error}"
    );
}

#[test]
fn an_endless_loop_is_stopped_by_the_work_budget() {
    let limits = Limits {
        polls: 50,
        ..Limits::default()
    };
    let error = run("while (true) {}", 1, &limits, never())
        .err()
        .unwrap_or_else(|| panic_free());
    assert!(error.stopped, "{error}");
}

#[test]
fn cancellation_stops_a_running_script() {
    let polled = Rc::new(Cell::new(0_u32));
    let seen = Rc::clone(&polled);
    let cancel: Rc<dyn Fn() -> bool> = Rc::new(move || {
        seen.set(seen.get() + 1);
        seen.get() > 3
    });
    let limits = Limits {
        polls: u64::MAX,
        ..Limits::default()
    };
    let error = run("while (true) {}", 1, &limits, cancel)
        .err()
        .unwrap_or_else(|| panic_free());
    assert!(error.stopped, "{error}");
    assert!(
        polled.get() > 3,
        "the engine asked the page whether to stop"
    );
}

#[test]
fn a_memory_bomb_is_refused() {
    let limits = Limits {
        memory: 8 * 1024 * 1024,
        polls: u64::MAX,
        ..Limits::default()
    };
    let error = run(
        "const keep = []; while (true) keep.push(new Array(100000).fill(1.5));",
        1,
        &limits,
        never(),
    )
    .err()
    .unwrap_or_else(|| panic_free());
    assert!(
        error.message.to_lowercase().contains("memory") || error.stopped,
        "{error}"
    );
}

#[test]
fn runaway_recursion_is_an_error_not_a_crash() {
    let error = go("function f(n) { return f(n + 1) + 1; } f(0);")
        .err()
        .unwrap_or_else(|| panic_free());
    assert!(
        error.message.contains("stack") || error.message.contains("recursion"),
        "{error}"
    );
}

fn panic_free() -> ve_modeling::script::ScriptError {
    ve_modeling::script::ScriptError {
        message: "expected an error but the script succeeded".to_owned(),
        line: None,
        column: None,
        stopped: false,
    }
}
