//! Runtime traversal and transition handling for FSMs.
use super::{Fsm, FsmTick, Machine, State, run_action, test};
use crate::{BrainError, Ctx, Registry};

pub(super) fn tick(
    fsm: &mut Fsm,
    registry: &mut Registry,
    ctx: &mut Ctx,
) -> Result<FsmTick, BrainError> {
    if !fsm.started {
        fsm.start(registry, ctx)?;
        return Ok(FsmTick {
            path: fsm.active.clone(),
            changed: true,
        });
    }
    let events = std::mem::take(&mut fsm.events);
    let mut owner = None;
    for index in (0..fsm.active.len()).rev() {
        let machine = machine_at(&fsm.root, &fsm.active, index)?;
        let name = &fsm.active[index];
        let state = machine
            .states
            .get(name)
            .ok_or_else(|| BrainError::BadSpec(format!("missing state {name}")))?;
        let entered = *fsm.entered.get(&fsm.active[..=index]).unwrap_or(&ctx.now);
        if let Some(transition) = state
            .transitions
            .iter()
            .find(|transition| test(&transition.when, registry, ctx, &events, entered))
        {
            owner = Some((index, transition.to.clone()));
            break;
        }
    }
    let changed = if let Some((index, target)) = owner {
        transition(fsm, index, &target, registry, ctx)?;
        true
    } else {
        update(fsm, registry, ctx)?;
        false
    };
    Ok(FsmTick {
        path: fsm.active.clone(),
        changed,
    })
}
fn machine_at<'a>(
    root: &'a Machine,
    path: &[String],
    index: usize,
) -> Result<&'a Machine, BrainError> {
    let mut machine = root;
    for name in path.iter().take(index) {
        let state = machine
            .states
            .get(name)
            .ok_or_else(|| BrainError::BadSpec(format!("missing state {name}")))?;
        machine = state
            .child
            .as_deref()
            .ok_or_else(|| BrainError::BadSpec(format!("state {name} has no children")))?;
    }
    Ok(machine)
}
fn state_at(root: &Machine, path: &[String], index: usize) -> Result<State, BrainError> {
    machine_at(root, path, index)?
        .states
        .get(&path[index])
        .cloned()
        .ok_or_else(|| BrainError::BadSpec(format!("missing state {}", path[index])))
}
fn transition(
    fsm: &mut Fsm,
    index: usize,
    target: &str,
    registry: &mut Registry,
    ctx: &mut Ctx,
) -> Result<(), BrainError> {
    while fsm.active.len() > index {
        let state = state_at(&fsm.root, &fsm.active, fsm.active.len() - 1)?;
        if let Some(action) = state.exit {
            run_action(registry, &action, ctx)?;
        }
        fsm.entered.remove(&fsm.active);
        fsm.active.pop();
    }
    let prefix = fsm.active.clone();
    let machine = machine_at(&fsm.root, &prefix, index)?;
    let state = machine
        .states
        .get(target)
        .ok_or_else(|| BrainError::BadSpec(format!("missing transition target {target}")))?
        .clone();
    fsm.active.push(target.to_owned());
    enter_one(fsm, &state, registry, ctx)?;
    let mut child = state.child;
    while let Some(current) = child {
        let name = current.initial.clone();
        let state = current
            .states
            .get(&name)
            .ok_or_else(|| BrainError::BadSpec(format!("missing state {name}")))?
            .clone();
        fsm.active.push(name);
        enter_one(fsm, &state, registry, ctx)?;
        child = state.child;
    }
    Ok(())
}
fn enter_one(
    fsm: &mut Fsm,
    state: &State,
    registry: &mut Registry,
    ctx: &mut Ctx,
) -> Result<(), BrainError> {
    fsm.entered.insert(fsm.active.clone(), ctx.now);
    if let Some(action) = &state.enter {
        run_action(registry, action, ctx)?;
    }
    Ok(())
}
fn update(fsm: &Fsm, registry: &mut Registry, ctx: &mut Ctx) -> Result<(), BrainError> {
    for index in 0..fsm.active.len() {
        let state = state_at(&fsm.root, &fsm.active, index)?;
        if let Some(action) = state.update {
            run_action(registry, &action, ctx)?;
        }
    }
    Ok(())
}
