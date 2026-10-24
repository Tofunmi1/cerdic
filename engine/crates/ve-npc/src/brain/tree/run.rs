use super::{BehaviorTree, BrainError, Kind, Node, Scalar, State};
use crate::brain::{Ctx, Status};
use crate::{Registry, Value};

pub(super) fn reset(node: &mut Node) {
    node.state = State::default();
    match &mut node.kind {
        Kind::Sequence(children) | Kind::Selector(children) | Kind::Parallel(_, children) => {
            for child in children {
                reset(child);
            }
        }
        Kind::Inverter(child)
        | Kind::Succeeder(child)
        | Kind::Repeat(_, child)
        | Kind::Cooldown(_, child) => reset(child),
        _ => {}
    }
}

pub(super) fn tick(
    tree: &mut BehaviorTree,
    registry: &mut Registry,
    ctx: &mut Ctx,
    budget: usize,
) -> Result<Status, BrainError> {
    tick_node(&mut tree.root, registry, ctx, &mut 0, budget)
}

fn finish(node: &mut Node, status: Status) -> Status {
    if status != Status::Running {
        let deadline = if matches!(&node.kind, Kind::Cooldown(_, _)) {
            node.state.cooldown_until
        } else {
            None
        };
        reset(node);
        node.state.cooldown_until = deadline;
    }
    status
}

fn tick_node(
    node: &mut Node,
    registry: &mut Registry,
    ctx: &mut Ctx,
    visits: &mut usize,
    budget: usize,
) -> Result<Status, BrainError> {
    *visits += 1;
    if *visits > budget {
        return Err(BrainError::TickBudget { budget });
    }
    let status = match &mut node.kind {
        Kind::Sequence(children) => {
            tick_sequence(children, &mut node.state, registry, ctx, visits, budget)?
        }
        Kind::Selector(children) => {
            tick_selector(children, &mut node.state, registry, ctx, visits, budget)?
        }
        Kind::Parallel(all, children) => tick_parallel(
            *all,
            children,
            &mut node.state,
            registry,
            ctx,
            visits,
            budget,
        )?,
        Kind::Inverter(child) => invert(tick_node(child, registry, ctx, visits, budget)?),
        Kind::Succeeder(child) => succeed(tick_node(child, registry, ctx, visits, budget)?),
        Kind::Repeat(times, child) => tick_repeat(
            *times,
            child,
            &mut node.state,
            registry,
            ctx,
            visits,
            budget,
        )?,
        Kind::Wait(secs) => tick_wait(*secs, &mut node.state, ctx),
        Kind::Cooldown(secs, child) => {
            tick_cooldown(*secs, child, &mut node.state, registry, ctx, visits, budget)?
        }
        Kind::Condition(name) => match registry.condition(name, ctx) {
            Some(true) => Status::Success,
            Some(false) => Status::Failure,
            None => return Err(BrainError::UnknownNames(vec![name.clone()])),
        },
        Kind::Action(name) => match registry.action(name, ctx) {
            Some(status) => status,
            None => return Err(BrainError::UnknownNames(vec![name.clone()])),
        },
        Kind::Check(key, op, expected) => {
            if check(ctx.blackboard.get(key, ctx.now), op, expected) {
                Status::Success
            } else {
                Status::Failure
            }
        }
        Kind::Set(key, value) => {
            ctx.blackboard.set(key, value.to_value(), ctx.now);
            Status::Success
        }
    };
    Ok(finish(node, status))
}

fn tick_sequence(
    children: &mut [Node],
    state: &mut State,
    registry: &mut Registry,
    ctx: &mut Ctx,
    visits: &mut usize,
    budget: usize,
) -> Result<Status, BrainError> {
    while state.index < children.len() {
        match tick_node(&mut children[state.index], registry, ctx, visits, budget)? {
            Status::Success => state.index += 1,
            other => return Ok(other),
        }
    }
    Ok(Status::Success)
}
fn tick_selector(
    children: &mut [Node],
    state: &mut State,
    registry: &mut Registry,
    ctx: &mut Ctx,
    visits: &mut usize,
    budget: usize,
) -> Result<Status, BrainError> {
    while state.index < children.len() {
        match tick_node(&mut children[state.index], registry, ctx, visits, budget)? {
            Status::Failure => state.index += 1,
            other => return Ok(other),
        }
    }
    Ok(Status::Failure)
}
fn tick_parallel(
    all: bool,
    children: &mut [Node],
    state: &mut State,
    registry: &mut Registry,
    ctx: &mut Ctx,
    visits: &mut usize,
    budget: usize,
) -> Result<Status, BrainError> {
    if state.parallel.len() != children.len() {
        state.parallel = vec![None; children.len()];
    }
    for (i, child) in children.iter_mut().enumerate() {
        if state.parallel[i].is_none() {
            let status = tick_node(child, registry, ctx, visits, budget)?;
            if status != Status::Running {
                state.parallel[i] = Some(status);
            }
        }
    }
    let values = state.parallel.iter().flatten().copied().collect::<Vec<_>>();
    if (all && values.contains(&Status::Failure)) || (!all && values.contains(&Status::Success)) {
        return Ok(if all {
            Status::Failure
        } else {
            Status::Success
        });
    }
    if values.len() == children.len() {
        return Ok(if all {
            Status::Success
        } else {
            Status::Failure
        });
    }
    Ok(Status::Running)
}
fn tick_repeat(
    times: usize,
    child: &mut Node,
    state: &mut State,
    registry: &mut Registry,
    ctx: &mut Ctx,
    visits: &mut usize,
    budget: usize,
) -> Result<Status, BrainError> {
    match tick_node(child, registry, ctx, visits, budget)? {
        Status::Failure => Ok(Status::Failure),
        Status::Running => Ok(Status::Running),
        Status::Success => {
            state.count += 1;
            Ok(if state.count >= times {
                Status::Success
            } else {
                Status::Running
            })
        }
    }
}
fn tick_wait(secs: f64, state: &mut State, ctx: &Ctx) -> Status {
    state.elapsed += ctx.dt;
    if state.elapsed >= secs {
        Status::Success
    } else {
        Status::Running
    }
}
fn tick_cooldown(
    secs: f64,
    child: &mut Node,
    state: &mut State,
    registry: &mut Registry,
    ctx: &mut Ctx,
    visits: &mut usize,
    budget: usize,
) -> Result<Status, BrainError> {
    if state.cooldown_until.is_some_and(|until| ctx.now < until) {
        return Ok(Status::Failure);
    }
    let status = tick_node(child, registry, ctx, visits, budget)?;
    if status != Status::Running {
        state.cooldown_until = Some(ctx.now + secs);
    }
    Ok(status)
}
fn invert(status: Status) -> Status {
    match status {
        Status::Success => Status::Failure,
        Status::Failure => Status::Success,
        Status::Running => Status::Running,
    }
}
fn succeed(status: Status) -> Status {
    if status == Status::Failure {
        Status::Success
    } else {
        status
    }
}
#[expect(
    clippy::float_cmp,
    reason = "blackboard number equality follows exact JSON numeric semantics"
)]
pub(crate) fn check(actual: Option<&Value>, op: &str, expected: &Scalar) -> bool {
    let Some(actual) = actual else { return false };
    match (actual, expected) {
        (Value::Number(a), Scalar::Number(b)) => match op {
            "<" => a < b,
            "<=" => a <= b,
            "==" => a == b,
            "!=" => a != b,
            ">=" => a >= b,
            ">" => a > b,
            _ => false,
        },
        (Value::Bool(a), Scalar::Bool(b)) => match op {
            "==" => a == b,
            "!=" => a != b,
            _ => false,
        },
        (Value::Text(a), Scalar::Text(b)) => match op {
            "==" => a == b,
            "!=" => a != b,
            _ => false,
        },
        _ => false,
    }
}
impl Scalar {
    fn to_value(&self) -> Value {
        match self {
            Self::Number(v) => Value::Number(*v),
            Self::Bool(v) => Value::Bool(*v),
            Self::Text(v) => Value::Text(v.clone()),
        }
    }
}
