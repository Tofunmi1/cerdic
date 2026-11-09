//! Rule and entity selection shared by simulation systems.
use super::{Entity, Event, World, follow::matches_selector};

impl World {
    pub(super) fn rule_matches(&self, rule: &crate::Rule, event: &Event) -> bool {
        if rule.event != event.name {
            return false;
        }
        if rule.event == "tick"
            && rule
                .every
                .is_some_and(|steps| !self.tick.is_multiple_of(steps))
        {
            return false;
        }
        if event.name == "start" {
            let scoped = rule.a.is_some() || rule.b.is_some();
            if scoped == event.a.is_none() {
                return false;
            }
        }
        self.selector_matches(rule.a.as_deref(), event.a)
            && self.selector_matches(rule.b.as_deref(), event.b)
    }

    fn selector_matches(&self, selector: Option<&str>, id: Option<u32>) -> bool {
        let Some(selector) = selector else {
            return true;
        };
        let Some(entity) = id.and_then(|id| self.entities.iter().find(|entity| entity.id == id))
        else {
            return false;
        };
        self.matches(selector, entity)
    }

    pub(super) fn matches(&self, selector: &str, entity: &Entity) -> bool {
        matches_selector(&self.def, selector, entity)
    }

    pub(super) fn lookup(&self, name: &str, entity_id: Option<u32>) -> f64 {
        self.vars
            .get(name)
            .copied()
            .or_else(|| {
                entity_id
                    .and_then(|id| self.entities.iter().find(|entity| entity.id == id))
                    .and_then(|entity| entity.props.get(name).copied())
            })
            .unwrap_or(0.0)
    }
}
