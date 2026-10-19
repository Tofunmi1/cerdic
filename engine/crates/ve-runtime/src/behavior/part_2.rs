
/// Takes what the hands picked up or let go this frame (play mode only).
///
/// A part let go within a socket's radius snaps onto it and docks (the socket fires with `true`); picking a docked
/// part up again undocks it (the socket fires with `false`).
pub(crate) fn collect_grabs(
    mut interaction: ResMut<InteractionState>,
    mut behaviors: ResMut<Behaviors>,
    mut parts: Query<(Entity, &Reactive, &mut Transform)>,
) {
    for (entity, grabbed) in interaction.take_grabs() {
        if !behaviors.play {
            continue;
        }
        if grabbed {
            behaviors.held.push(entity);
            behaviors.grabs.push(entity);
            if let Some(at) = behaviors
                .docked
                .iter()
                .position(|(part, _)| *part == entity)
            {
                let (_, socket) = behaviors.docked.remove(at);
                behaviors.docks.push((socket, false));
            }
            continue;
        }
        behaviors.releases.push(entity);
        behaviors.held.retain(|held| *held != entity);
        let Ok((_, released, transform)) = parts.get(entity) else {
            continue;
        };
        let (name, here) = (
            released.name.clone(),
            released.centre + transform.translation,
        );
        let fitting = parts
            .iter()
            .filter(|(_, socket, _)| {
                let behavior = &socket.reaction.behavior;
                behavior.trigger == Trigger::Socket
                    && behavior.accepts.as_deref() == Some(name.as_str())
                    && socket.centre.distance(here) <= behavior.radius
            })
            .min_by(|(_, a, _), (_, b, _)| {
                a.centre.distance(here).total_cmp(&b.centre.distance(here))
            })
            .map(|(socket_entity, socket, _)| (socket_entity, socket.centre));
        if let Some((socket_entity, centre)) = fitting
            && let Ok((_, _, mut transform)) = parts.get_mut(entity)
        {
            // Snap: the part's centre goes onto the socket's centre.
            transform.translation += centre - here;
            behaviors
                .docked
                .retain(|(_, socket)| *socket != socket_entity);
            behaviors.docked.push((entity, socket_entity));
            behaviors.docks.push((socket_entity, true));
        }
    }
}

/// While playing, parts that react to being grabbed can be picked up; otherwise authored parts stay put.
pub(crate) fn sync_grabbable(
    behaviors: Res<Behaviors>,
    mut parts: Query<
        (
            Option<&Reactive>,
            Option<&crate::physics::Body>,
            &mut Interactable,
        ),
        // Only parts that react or are bodies: other scene objects keep whatever grabbing they were built with.
        Or<(With<Reactive>, With<crate::physics::Body>)>,
    >,
) {
    for (part, body, mut interactable) in &mut parts {
        // A physics prop (anything that is not a static body) can be picked up and thrown.
        let prop = body.is_some_and(|body| body.0.is_movable());
        let reacts = part.is_some_and(|part| {
            matches!(
                part.reaction.behavior.trigger,
                Trigger::Grab | Trigger::Release | Trigger::Hold
            )
        });
        interactable.set_grabbable(behaviors.play && (reacts || prop));
    }
}

/// Queues a trigger when the camera enters a `near` part's radius (and, for a toggle, when it leaves).
pub(crate) fn proximity(
    mut behaviors: ResMut<Behaviors>,
    camera: Query<&GlobalTransform, With<Camera3d>>,
    mut parts: Query<(Entity, &mut Reactive)>,
) {
    if !behaviors.play {
        return;
    }
    let Some(eye) = camera.iter().next().map(GlobalTransform::translation) else {
        return;
    };
    for (entity, mut part) in &mut parts {
        let behavior = &part.reaction.behavior;
        if behavior.trigger != Trigger::Near {
            continue;
        }
        let (radius, toggles) = (behavior.radius, behavior.action == Action::Toggle);
        let inside = eye.distance(part.centre) <= radius;
        if inside != part.inside {
            part.inside = inside;
            if inside || toggles {
                behaviors.pending.push(entity);
            }
        }
    }
}

/// Runs whole fixed steps for the real time that passed, applying queued triggers at the first step.
pub(crate) fn run(
    time: Res<Time<Real>>,
    clock: Res<crate::animation::AnimationClock>,
    mut behaviors: ResMut<Behaviors>,
    mut parts: Query<(Entity, &mut Reactive)>,
) {
    behaviors.stepped = 0;
    if behaviors.play {
        let cap = STEP * MAX_STEPS_PER_FRAME;
        behaviors.accumulator = (behaviors.accumulator + time.delta_secs()).min(cap);
        while behaviors.accumulator >= STEP {
            behaviors.accumulator -= STEP;
            step::step(&mut behaviors, &mut parts);
            behaviors.stepped += 1;
        }
    }
    let listed: Vec<_> = parts
        .iter()
        .map(|(_, part)| {
            let clips: serde_json::Map<String, serde_json::Value> = part
                .clips
                .iter()
                .map(|(name, r)| (name.clone(), json!({"time": r.time, "forward": r.forward, "running": r.running})))
                .collect();
            json!({"node": part.node, "time": part.reaction.time,
                   "forward": part.reaction.forward, "running": part.reaction.running, "clips": clips})
        })
        .collect();
    let text = json!({"play": behaviors.play, "steps": behaviors.steps, "clicks": behaviors.clicks_seen,
                      "flags": behaviors.flags, "parts": listed,
                      "clock": {"seconds": clock.seconds, "playing": clock.playing, "speed": clock.speed}})
    .to_string();
    STATE.with(|s| *s.borrow_mut() = text);
}
