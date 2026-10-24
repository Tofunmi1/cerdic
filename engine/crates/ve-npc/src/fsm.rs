//! Data-driven hierarchical finite state machines.
use std::collections::{BTreeMap, BTreeSet};

use crate::brain::tree::{Scalar, run::check};
use crate::{BrainError, Ctx, Registry, Status};

mod parse;
mod tick;

/// Result of one state machine tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FsmTick {
    /// Active path from outermost to innermost state.
    pub path: Vec<String>,
    /// Whether a transition was taken.
    pub changed: bool,
}

#[derive(Clone)]
pub(super) struct Machine {
    pub initial: String,
    pub states: BTreeMap<String, State>,
}
#[derive(Clone)]
pub(super) struct State {
    pub enter: Option<String>,
    pub update: Option<String>,
    pub exit: Option<String>,
    pub transitions: Vec<Transition>,
    pub child: Option<Box<Machine>>,
}
#[derive(Clone)]
pub(super) struct Transition {
    pub to: String,
    pub when: When,
}
#[derive(Clone)]
pub(super) enum When {
    Condition(String),
    Check(String, String, Scalar),
    After(f64),
    Event(String),
    Not(Box<When>),
    All(Vec<When>),
    Any(Vec<When>),
}

/// A parsed hierarchical finite state machine.
pub struct Fsm {
    pub(super) root: Machine,
    pub(super) active: Vec<String>,
    pub(super) entered: BTreeMap<Vec<String>, f64>,
    pub(super) events: BTreeSet<String>,
    pub(super) started: bool,
}

impl Fsm {
    /// Parses a state machine JSON description.
    ///
    /// # Errors
    /// Returns `BadSpec` when the JSON or state hierarchy is invalid.
    pub fn from_json(text: &str) -> Result<Self, BrainError> {
        Self::from_json_with_library(text, &BTreeMap::new())
    }
    /// Parses a machine and resolves named linked machines.
    ///
    /// # Errors
    /// Returns `BadSpec` for malformed machines, missing links, or link cycles.
    pub fn from_json_with_library(
        text: &str,
        library: &BTreeMap<String, String>,
    ) -> Result<Self, BrainError> {
        Ok(Self {
            root: parse::parse(text, library)?,
            active: Vec::new(),
            entered: BTreeMap::new(),
            events: BTreeSet::new(),
            started: false,
        })
    }
    /// Serializes the machine in stable JSON form.
    #[must_use]
    pub fn to_json(&self) -> String {
        parse::serialize(&self.root)
    }
    /// Checks every registered condition and action name used by the machine.
    ///
    /// # Errors
    /// Returns `UnknownNames` with every unregistered name.
    pub fn validate(&self, registry: &Registry) -> Result<(), BrainError> {
        let mut names = BTreeSet::new();
        parse::collect(&self.root, registry, &mut names);
        if names.is_empty() {
            Ok(())
        } else {
            Err(BrainError::UnknownNames(names.into_iter().collect()))
        }
    }
    /// Enters the initial state path and runs enter actions.
    ///
    /// # Errors
    /// Returns `UnknownNames` if an enter action is not registered.
    pub fn start(&mut self, registry: &mut Registry, ctx: &mut Ctx) -> Result<(), BrainError> {
        self.active.clear();
        self.entered.clear();
        self.events.clear();
        self.started = true;
        let mut machine = &self.root;
        loop {
            let name = machine.initial.clone();
            self.active.push(name.clone());
            self.entered.insert(self.active.clone(), ctx.now);
            let state = machine
                .states
                .get(&name)
                .ok_or_else(|| BrainError::BadSpec(format!("missing state {name}")))?;
            if let Some(action) = &state.enter {
                run_action(registry, action, ctx)?;
            }
            if let Some(child) = &state.child {
                machine = child;
            } else {
                break;
            }
        }
        Ok(())
    }
    /// Evaluates one transition or updates every active state.
    ///
    /// # Errors
    /// Returns `UnknownNames` for an unregistered action or condition.
    pub fn tick(&mut self, registry: &mut Registry, ctx: &mut Ctx) -> Result<FsmTick, BrainError> {
        tick::tick(self, registry, ctx)
    }
    /// Queues an event for the next tick.
    pub fn send_event(&mut self, event: &str) {
        if self.started {
            self.events.insert(event.to_owned());
        }
    }
    /// Returns the active outer-to-inner state path.
    #[must_use]
    pub fn state_path(&self) -> Vec<String> {
        self.active.clone()
    }
}

pub(super) fn run_action(
    registry: &mut Registry,
    name: &str,
    ctx: &mut Ctx,
) -> Result<(), BrainError> {
    registry
        .action(name, ctx)
        .map(|_: Status| ())
        .ok_or_else(|| BrainError::UnknownNames(vec![name.to_owned()]))
}
pub(super) fn test(
    when: &When,
    registry: &Registry,
    ctx: &Ctx,
    events: &BTreeSet<String>,
    entered: f64,
) -> bool {
    match when {
        When::Condition(name) => registry.condition(name, ctx).unwrap_or(false),
        When::Check(key, op, expected) => check(ctx.blackboard.get(key, ctx.now), op, expected),
        When::After(secs) => ctx.now - entered >= *secs,
        When::Event(name) => events.contains(name),
        When::Not(inner) => !test(inner, registry, ctx, events, entered),
        When::All(items) => items
            .iter()
            .all(|v| test(v, registry, ctx, events, entered)),
        When::Any(items) => items
            .iter()
            .any(|v| test(v, registry, ctx, events, entered)),
    }
}
