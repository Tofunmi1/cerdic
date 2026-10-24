use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::brain::tree::Scalar;
use crate::{BrainError, Registry, Status, Value};

const MAX_PLANNER_DEPTH: usize = 256;
mod planner;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Check {
    key: String,
    op: String,
    value: ScalarValue,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum ScalarValue {
    Number(f64),
    Bool(bool),
    Text(String),
}
impl From<&ScalarValue> for Scalar {
    fn from(v: &ScalarValue) -> Self {
        match v {
            ScalarValue::Number(x) => Self::Number(*x),
            ScalarValue::Bool(x) => Self::Bool(*x),
            ScalarValue::Text(x) => Self::Text(x.clone()),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum Cond {
    Utility { utility: Utility },
    Check(Check),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Utility {
    option: String,
    min: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum Effect {
    Set { set: SetEffect },
    Add { add: AddEffect },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct SetEffect {
    key: String,
    value: ScalarValue,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct AddEffect {
    key: String,
    amount: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Method {
    name: String,
    #[serde(default)]
    when: Vec<Cond>,
    #[serde(rename = "do")]
    tasks: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum Task {
    Primitive {
        primitive: String,
        #[serde(default)]
        pre: Vec<Check>,
        #[serde(default)]
        effects: Vec<Effect>,
    },
    Compound {
        methods: Vec<Method>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Spec {
    tasks: BTreeMap<String, Task>,
}

/// A hierarchical task network domain.
#[derive(Clone, Debug)]
pub struct Domain {
    spec: Spec,
}
/// Limits on task expansion during planning.
#[derive(Clone, Debug)]
pub struct PlanLimits {
    /// Maximum nested task depth.
    pub max_depth: usize,
    /// Maximum task expansions attempted.
    pub max_nodes: usize,
}
impl Default for PlanLimits {
    fn default() -> Self {
        Self {
            max_depth: 32,
            max_nodes: 10_000,
        }
    }
}
/// A primitive action in a plan.
#[derive(Clone, Debug)]
pub struct Step {
    /// Domain task name.
    pub task: String,
    /// Registered action name.
    pub action: String,
}
/// A plan and its predicted final state.
#[derive(Clone, Debug)]
pub struct Plan {
    /// Ordered primitive steps.
    pub steps: Vec<Step>,
    /// State after applying all planned effects.
    pub final_state: BTreeMap<String, Value>,
}
/// Planning failures.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum HtnError {
    /// No valid decomposition exists.
    #[error("no plan for {0}")]
    NoPlan(String),
    /// A configured limit was exceeded.
    #[error("planning limit exceeded: {0}")]
    Limit(String),
    /// Goal or subtask is undefined.
    #[error("unknown task {0}")]
    UnknownTask(String),
}
impl Domain {
    /// Parses and validates a domain definition.
    ///
    /// # Errors
    /// Returns `BadSpec` for invalid JSON or task definitions.
    pub fn from_json(text: &str) -> Result<Self, BrainError> {
        let spec: Spec =
            serde_json::from_str(text).map_err(|e| BrainError::BadSpec(e.to_string()))?;
        for (name, task) in &spec.tasks {
            if name.trim().is_empty() {
                return Err(BrainError::BadSpec("empty task name".into()));
            }
            if let Task::Compound { methods } = task {
                if methods.is_empty() {
                    return Err(BrainError::BadSpec(format!(
                        "compound task {name} has no methods"
                    )));
                }
                for m in methods {
                    if m.name.trim().is_empty() {
                        return Err(BrainError::BadSpec("empty method name".into()));
                    }
                    for t in &m.tasks {
                        if !spec.tasks.contains_key(t) {
                            return Err(BrainError::BadSpec(format!("unknown task {t}")));
                        }
                    }
                    for c in &m.when {
                        if let Cond::Check(ch) = c {
                            validate_check(ch)?;
                        }
                    }
                }
            }
            if let Task::Primitive { pre, .. } = task {
                for c in pre {
                    validate_check(c)?;
                }
            }
        }
        Ok(Self { spec })
    }
    /// Serializes the domain definition.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.spec).unwrap_or_default()
    }
    /// Checks that every primitive action is registered.
    ///
    /// # Errors
    /// Returns `UnknownNames` listing every missing action.
    pub fn validate(&self, registry: &Registry) -> Result<(), BrainError> {
        let missing: Vec<_> = self
            .spec
            .tasks
            .values()
            .filter_map(|t| {
                if let Task::Primitive { primitive, .. } = t {
                    (!registry.has_action(primitive)).then(|| primitive.clone())
                } else {
                    None
                }
            })
            .collect();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(BrainError::UnknownNames(missing))
        }
    }
}
fn validate_check(c: &Check) -> Result<(), BrainError> {
    if !["<", "<=", "==", "!=", ">=", ">"].contains(&c.op.as_str()) {
        Err(BrainError::BadSpec(format!(
            "invalid check operator {}",
            c.op
        )))
    } else if c.key.trim().is_empty() {
        Err(BrainError::BadSpec("empty check key".into()))
    } else {
        Ok(())
    }
}
fn check(state: &BTreeMap<String, Value>, c: &Check) -> bool {
    crate::brain::tree::run::check(state.get(&c.key), &c.op, &Scalar::from(&c.value))
}
fn apply(state: &mut BTreeMap<String, Value>, e: &Effect) {
    match e {
        Effect::Set { set } => {
            let v = match &set.value {
                ScalarValue::Number(x) => Value::Number(*x),
                ScalarValue::Bool(x) => Value::Bool(*x),
                ScalarValue::Text(x) => Value::Text(x.clone()),
            };
            state.insert(set.key.clone(), v);
        }
        Effect::Add { add } => {
            let current = match state.get(&add.key) {
                Some(Value::Number(x)) => *x,
                _ => 0.0,
            };
            state.insert(add.key.clone(), Value::Number(current + add.amount));
        }
    }
}

/// State of an executing plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunStatus {
    /// An action is still running.
    Running,
    /// All actions completed.
    Done,
    /// An action failed at this index.
    Failed {
        /// Failed plan step index.
        step: usize,
    },
}
/// Executes planned actions through a registry.
pub struct PlanRunner {
    plan: Plan,
    index: usize,
    failed: Option<usize>,
}
impl PlanRunner {
    /// Creates a runner for a plan.
    #[must_use]
    pub fn new(plan: Plan) -> Self {
        Self {
            plan,
            index: 0,
            failed: None,
        }
    }
    /// Advances registered actions until one runs, one fails, or the plan finishes.
    ///
    /// # Errors
    /// Returns `UnknownNames` if the current step's action is not registered.
    pub fn tick(
        &mut self,
        registry: &mut Registry,
        ctx: &mut crate::Ctx,
    ) -> Result<RunStatus, BrainError> {
        if let Some(step) = self.failed {
            return Ok(RunStatus::Failed { step });
        }
        while self.index < self.plan.steps.len() {
            let i = self.index;
            let action = &self.plan.steps[i].action;
            match registry.action(action, ctx) {
                Some(Status::Running) => return Ok(RunStatus::Running),
                Some(Status::Success) => self.index += 1,
                Some(Status::Failure) => {
                    self.failed = Some(i);
                    return Ok(RunStatus::Failed { step: i });
                }
                None => return Err(BrainError::UnknownNames(vec![action.clone()])),
            }
        }
        Ok(RunStatus::Done)
    }
}
