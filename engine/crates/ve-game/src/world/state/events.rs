//! Dispatching events and counting rule applications.
use super::{Event, MAX_ACTIONS, World};
use crate::{Effect, GameError};
use std::collections::BTreeSet;

impl World {
    pub(super) fn collect_touches(&mut self, queue: &mut Vec<Event>) {
        let mut next = BTreeSet::new();
        for i in 0..self.entities.len() {
            for j in i + 1..self.entities.len() {
                let first = &self.entities[i];
                let second = &self.entities[j];
                let radius = |name: &str| {
                    self.def
                        .kinds
                        .iter()
                        .find(|kind| kind.name == name)
                        .map_or(0.5, |kind| kind.size)
                };
                let distance = (0..3)
                    .map(|axis| (first.position[axis] - second.position[axis]).powi(2))
                    .sum::<f64>();
                if distance < (radius(&first.kind) + radius(&second.kind)).powi(2) {
                    let pair = (first.id, second.id);
                    if !self.overlaps.contains(&pair) {
                        let (a, b) = (first.id, second.id);
                        queue.push(Event {
                            name: "touch".into(),
                            a: Some(a),
                            b: Some(b),
                            depth: 0,
                        });
                        queue.push(Event {
                            name: "touch".into(),
                            a: Some(b),
                            b: Some(a),
                            depth: 0,
                        });
                    }
                    next.insert(pair);
                }
            }
        }
        self.overlaps = next;
    }

    pub(super) fn process_events(
        &mut self,
        queue: &mut Vec<Event>,
        effects: &mut Vec<Effect>,
    ) -> Result<usize, GameError> {
        let mut actions = 0;
        while let Some(event) = queue.first().cloned() {
            queue.remove(0);
            if event.depth > 8 {
                return Err(GameError::ChainDepth(8));
            }
            // `reap:ID` removes an entity that died or was used up, after the rules on `killed` and `impact` have run
            if let Some(id) = event.name.strip_prefix("reap:") {
                let id = id.parse::<u32>().unwrap_or(u32::MAX);
                self.entities.retain(|entity| entity.id != id);
                self.agents.retain(|agent, _| *agent != id);
                continue;
            }
            // every rule on one event sees the flow state the event began in, so `pause` can toggle: one rule leaves `playing`, another
            // enters it from `paused`, and the first one's change is not heard by the second
            let state_then = self.flow.state.clone();
            for rule_index in 0..self.def.rules.len() {
                let rule = &self.def.rules[rule_index];
                if rule.state.as_ref().is_some_and(|state| *state != state_then) || !self.rule_matches(rule, &event) {
                    continue;
                }
                if let Some(expr) = &rule.expr
                    && expr.eval(&|name| self.lookup(name, event.a)) == 0.0
                {
                    continue;
                }
                if !rule.actions.is_empty() {
                    self.rule_fires[rule_index] = self.rule_fires[rule_index].saturating_add(1);
                }
                let rule_actions = rule.actions.clone();
                for action in rule_actions {
                    actions += 1;
                    if actions > MAX_ACTIONS {
                        return Err(GameError::ActionBudget(MAX_ACTIONS));
                    }
                    self.action(&action, &event, queue, effects)?;
                }
            }
            actions += self.script_event(&event, queue, effects)?;
        }
        Ok(actions)
    }
}
