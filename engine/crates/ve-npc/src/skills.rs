//! Prioritized interruptible skills backed by behaviour trees.
use crate::{BehaviorTree, BrainError, Ctx, Registry, Status};
use std::collections::BTreeSet;

/// One skill's selection and execution result.
pub struct SkillTick {
    /// Chosen skill name, if any.
    pub active: Option<String>,
    /// Active behaviour status.
    pub status: Status,
    /// Whether the selected skill changed.
    pub switched: bool,
}
struct Skill {
    name: String,
    priority: u8,
    when: Option<String>,
    tree: BehaviorTree,
}
/// An ordered set of interruptible behaviour skills.
#[derive(Default)]
pub struct SkillStack {
    skills: Vec<Skill>,
    active: Option<usize>,
}
impl SkillStack {
    /// Creates an empty skill stack.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Adds a skill with optional condition and behaviour tree JSON.
    ///
    /// # Errors
    /// Returns `BadSpec` for duplicate names or malformed tree JSON.
    pub fn add(
        &mut self,
        name: &str,
        priority: u8,
        when: Option<&str>,
        tree_json: &str,
    ) -> Result<(), BrainError> {
        if self.skills.iter().any(|skill| skill.name == name) {
            return Err(BrainError::BadSpec(format!("duplicate skill {name}")));
        }
        let tree = BehaviorTree::from_json(tree_json)?;
        self.skills.push(Skill {
            name: name.to_owned(),
            priority,
            when: when.map(str::to_owned),
            tree,
        });
        Ok(())
    }
    /// Checks all condition and action names used by every skill.
    ///
    /// # Errors
    /// Returns `UnknownNames` with every unregistered name.
    pub fn validate(&self, registry: &Registry) -> Result<(), BrainError> {
        let mut names = BTreeSet::new();
        for skill in &self.skills {
            if let Some(name) = &skill.when
                && !registry.has_condition(name)
            {
                names.insert(name.clone());
            }
            if let Err(BrainError::UnknownNames(found)) = skill.tree.validate(registry) {
                names.extend(found);
            }
        }
        if names.is_empty() {
            Ok(())
        } else {
            Err(BrainError::UnknownNames(names.into_iter().collect()))
        }
    }
    /// Selects the highest priority matching skill and ticks its tree.
    ///
    /// # Errors
    /// Returns an error from the selected behaviour tree.
    pub fn tick(
        &mut self,
        registry: &mut Registry,
        ctx: &mut Ctx,
    ) -> Result<SkillTick, BrainError> {
        let selected = self
            .skills
            .iter()
            .enumerate()
            .filter(|(_, skill)| {
                skill
                    .when
                    .as_ref()
                    .is_none_or(|name| registry.condition(name, ctx).unwrap_or(false))
            })
            .fold(None, |best, (index, skill)| match best {
                None => Some(index),
                Some(previous) if skill.priority > self.skills[previous].priority => Some(index),
                other => other,
            });
        let switched = selected != self.active;
        if switched {
            if let Some(previous) = self.active {
                self.skills[previous].tree.reset();
            }
            self.active = selected;
        }
        let status = if let Some(index) = selected {
            self.skills[index].tree.tick(registry, ctx)?
        } else {
            Status::Failure
        };
        Ok(SkillTick {
            active: selected.map(|i| self.skills[i].name.clone()),
            status,
            switched,
        })
    }
}
