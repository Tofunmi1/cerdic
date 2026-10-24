use std::collections::BTreeMap;

use serde_json::Value as Json;

use super::{BrainError, Ctx, Status};
use crate::Registry;

pub(crate) mod run;
mod spec;
mod validate;

const MAX_DEPTH: usize = 64;

/// A parsed and stateful behaviour tree.
pub struct BehaviorTree {
    pub(super) root: Node,
    source: Json,
    budget: usize,
}

pub(super) struct Node {
    pub(super) kind: Kind,
    pub(super) state: State,
}

pub(super) enum Kind {
    Sequence(Vec<Node>),
    Selector(Vec<Node>),
    Parallel(bool, Vec<Node>),
    Inverter(Box<Node>),
    Succeeder(Box<Node>),
    Repeat(usize, Box<Node>),
    Wait(f64),
    Cooldown(f64, Box<Node>),
    Condition(String),
    Action(String),
    Check(String, String, Scalar),
    Set(String, Scalar),
}

#[derive(Clone)]
pub(crate) enum Scalar {
    Number(f64),
    Bool(bool),
    Text(String),
}

#[derive(Default)]
pub(super) struct State {
    pub(super) index: usize,
    pub(super) parallel: Vec<Option<Status>>,
    pub(super) count: usize,
    pub(super) elapsed: f64,
    pub(super) cooldown_until: Option<f64>,
}

impl BehaviorTree {
    /// Builds a tree from JSON. Nesting is limited to 64 nodes to keep parsing and execution stack bounded.
    ///
    /// # Errors
    /// Returns `BadSpec` when JSON or a node definition is invalid.
    pub fn from_json(text: &str) -> Result<Self, BrainError> {
        Self::from_json_with_library(text, &BTreeMap::new())
    }

    /// Builds a tree and resolves named subtree JSON from a library.
    ///
    /// # Errors
    /// Returns `BadSpec` for invalid specs or missing subtrees and `Cycle` for recursive references.
    pub fn from_json_with_library(
        text: &str,
        library: &BTreeMap<String, String>,
    ) -> Result<Self, BrainError> {
        let source: Json =
            serde_json::from_str(text).map_err(|error| BrainError::BadSpec(error.to_string()))?;
        let root = spec::build_root(&source, library)?;
        Ok(Self {
            root,
            source,
            budget: 10_000,
        })
    }

    /// Returns stable JSON for the parsed tree.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.source).unwrap_or_else(|_| "null".to_owned())
    }

    /// Checks all condition and action names against a registry.
    ///
    /// # Errors
    /// Returns `UnknownNames` containing every unregistered condition or action.
    pub fn validate(&self, registry: &Registry) -> Result<(), BrainError> {
        let names = validate::unknown_names(&self.root, registry);
        if names.is_empty() {
            Ok(())
        } else {
            Err(BrainError::UnknownNames(names))
        }
    }

    /// Runs the tree for one fixed simulation step.
    ///
    /// # Errors
    /// Returns `UnknownNames` for an unregistered node or `TickBudget` when the visit limit is exceeded.
    pub fn tick(&mut self, registry: &mut Registry, ctx: &mut Ctx) -> Result<Status, BrainError> {
        run::tick(self, registry, ctx, self.budget)
    }

    /// Resets all node run state.
    pub fn reset(&mut self) {
        run::reset(&mut self.root);
    }

    /// Sets the maximum number of node visits allowed per tick.
    pub fn set_node_budget(&mut self, budget: usize) {
        self.budget = budget;
    }
}
