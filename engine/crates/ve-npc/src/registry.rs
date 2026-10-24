use std::collections::BTreeMap;

use crate::brain::{Ctx, Status};

/// Name registry for behaviour tree conditions and actions.
#[derive(Default)]
pub struct Registry {
    conditions: BTreeMap<String, Box<dyn Fn(&Ctx) -> bool>>,
    actions: BTreeMap<String, Box<dyn FnMut(&mut Ctx) -> Status>>,
}

impl Registry {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Registers or replaces a condition.
    pub fn register_condition(&mut self, name: &str, f: impl Fn(&Ctx) -> bool + 'static) {
        self.conditions.insert(name.to_owned(), Box::new(f));
    }
    /// Registers or replaces a stateful action.
    pub fn register_action(&mut self, name: &str, f: impl FnMut(&mut Ctx) -> Status + 'static) {
        self.actions.insert(name.to_owned(), Box::new(f));
    }
    /// Returns whether a condition is registered.
    #[must_use]
    pub fn has_condition(&self, name: &str) -> bool {
        self.conditions.contains_key(name)
    }
    /// Returns whether an action is registered.
    #[must_use]
    pub fn has_action(&self, name: &str) -> bool {
        self.actions.contains_key(name)
    }
    pub(crate) fn condition(&self, name: &str, ctx: &Ctx) -> Option<bool> {
        self.conditions.get(name).map(|f| f(ctx))
    }
    pub(crate) fn action(&mut self, name: &str, ctx: &mut Ctx) -> Option<Status> {
        self.actions.get_mut(name).map(|f| f(ctx))
    }
}
