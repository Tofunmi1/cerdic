use std::collections::BTreeMap;

use super::{
    Cond, Domain, HtnError, MAX_PLANNER_DEPTH, Plan, PlanLimits, Step, Task, apply, check,
};
use crate::Value;

impl Domain {
    /// Finds the first complete decomposition of a goal.
    ///
    /// # Errors
    /// Returns `UnknownTask`, `NoPlan`, or `Limit` when planning cannot complete.
    pub fn plan(
        &self,
        state: &BTreeMap<String, Value>,
        goal: &str,
        limits: &PlanLimits,
        scorer: &dyn Fn(&str) -> f64,
    ) -> Result<Plan, HtnError> {
        if !self.spec.tasks.contains_key(goal) {
            return Err(HtnError::UnknownTask(goal.into()));
        }
        let mut budget = 0;
        let mut steps = Vec::new();
        let mut final_state = state.clone();
        if self.expand(
            goal,
            state,
            0,
            limits,
            &mut budget,
            scorer,
            &mut steps,
            &mut final_state,
        )? {
            Ok(Plan { steps, final_state })
        } else {
            Err(HtnError::NoPlan(goal.into()))
        }
    }

    fn expand(
        &self,
        name: &str,
        state: &BTreeMap<String, Value>,
        depth: usize,
        lim: &PlanLimits,
        nodes: &mut usize,
        score: &dyn Fn(&str) -> f64,
        steps: &mut Vec<Step>,
        out: &mut BTreeMap<String, Value>,
    ) -> Result<bool, HtnError> {
        let depth_limit = lim.max_depth.min(MAX_PLANNER_DEPTH);
        if depth >= depth_limit {
            return Err(HtnError::Limit(format!("depth {depth_limit}")));
        }
        *nodes += 1;
        if *nodes > lim.max_nodes {
            return Err(HtnError::Limit(format!("nodes {}", lim.max_nodes)));
        }
        match self
            .spec
            .tasks
            .get(name)
            .ok_or_else(|| HtnError::UnknownTask(name.into()))?
        {
            Task::Primitive {
                primitive,
                pre,
                effects,
            } => {
                if !pre.iter().all(|c| check(state, c)) {
                    return Ok(false);
                }
                let mut next = state.clone();
                for effect in effects {
                    apply(&mut next, effect);
                }
                steps.push(Step {
                    task: name.into(),
                    action: primitive.clone(),
                });
                *out = next;
                Ok(true)
            }
            Task::Compound { methods } => {
                for method in methods {
                    if !method.when.iter().all(|condition| match condition {
                        Cond::Check(spec) => check(state, spec),
                        Cond::Utility { utility } => score(&utility.option) >= utility.min,
                    }) {
                        continue;
                    }
                    let mut current = state.clone();
                    let mut branch_steps = Vec::new();
                    let mut applicable = true;
                    for subtask in &method.tasks {
                        let mut next = current.clone();
                        if !self.expand(
                            subtask,
                            &current,
                            depth + 1,
                            lim,
                            nodes,
                            score,
                            &mut branch_steps,
                            &mut next,
                        )? {
                            applicable = false;
                            break;
                        }
                        current = next;
                    }
                    if applicable {
                        steps.extend(branch_steps);
                        *out = current;
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        }
    }
}
