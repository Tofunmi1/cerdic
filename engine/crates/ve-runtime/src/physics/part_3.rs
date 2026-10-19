fn advance(
    behaviors: &mut Behaviors,
    physics: &mut Physics,
    parts: &mut Query<(Entity, &Body, &mut Transform)>,
    rig: &mut ViewerRig,
) {
    physics.entities = parts.iter().count();
    physics.movable = parts
        .iter()
        .filter(|(_, body, _)| body.0.is_movable())
        .count();
    if !behaviors.is_playing() {
        if physics.world.take().is_some() {
            for live in std::mem::take(&mut physics.bodies) {
                for entity in live.entities {
                    if let Ok((_, _, mut transform)) = parts.get_mut(entity) {
                        *transform = Transform::IDENTITY;
                    }
                }
            }
            physics.steps = 0;
            physics.marks.clear();
            physics.touching.clear();
        }
        if physics.walker.take().is_some() {
            rig.set_walking(false);
        }
        return;
    }
    if physics.world.is_none() {
        let settings = scene_settings(parts);
        let gravity = narrow(settings.as_ref().map_or(EARTH, |s| s.gravity));
        let Some((mut world, bodies)) = build(parts, gravity, &mut physics.refused) else {
            return;
        };
        physics.walker = settings
            .as_ref()
            .and_then(|settings| start_walker(&mut world, settings, rig, &bodies));
        physics.world = Some(world);
        physics.bodies = bodies;
    }
    let Physics {
        world,
        bodies,
        steps,
        marks,
        touching,
        walker,
        ..
    } = physics;
    let Some(world) = world.as_mut() else {
        return;
    };
    for _ in 0..behaviors.stepped() {
        if let Some(walking) = walker.as_mut() {
            walk_step(world, walking, rig);
        }
        for live in bodies
            .iter_mut()
            .filter(|live| live.kind != BodyKind::Static)
        {
            let hand = live
                .entities
                .iter()
                .find(|entity| behaviors.is_held(**entity))
                .copied();
            follow_hand(
                live,
                hand.and_then(|entity| parts.get(entity).ok().map(|(_, _, transform)| *transform)),
                hand.is_some(),
                world,
            );
        }
        world.step();
        *steps += 1;
        // Every simulated second, note where everything is (the first few only), so a test can compare two plays at the
        // same step however the frames fell.
        if *steps % 60 == 0 && marks.len() < MAX_MARKS {
            let poses = bodies
                .iter()
                .filter_map(|live| world.pose(live.id).ok())
                .map(|pose| pose.position)
                .collect();
            marks.push((*steps, poses));
        }
        // A touch that was not there after the last step is a collision for `collide` triggers.
        let now = world.contacts();
        let name_of = |id: BodyId| {
            bodies
                .iter()
                .find(|live| live.id == id)
                .map_or("", |live| live.spec.name.as_str())
        };
        for pair in now.iter().filter(|pair| !touching.contains(pair)) {
            behaviors.note_collision(name_of(pair.0), name_of(pair.1));
        }
        *touching = now;
    }
    if let Some(walking) = walker.as_ref() {
        rig.set_walking(true);
        if let Some(state) = walking.state {
            rig.place_eye(
                Vec3::from_array(state.position) + Vec3::Y * (walking.height * 0.5 - EYE_DROP),
            );
        }
    }
    for live in bodies.iter() {
        if live.kind == BodyKind::Static || live.held {
            continue;
        }
        let Ok(pose) = world.pose(live.id) else {
            continue;
        };
        let target = transform_for(live.centre, pose.position, pose.rotation);
        for entity in &live.entities {
            if let Ok((_, _, mut transform)) = parts.get_mut(*entity)
                && *transform != target
            {
                *transform = target;
            }
        }
    }
}

thread_local! {
    static PUBLISHED: std::cell::RefCell<String> = std::cell::RefCell::new(String::from("{\"active\":false,\"steps\":0,\"bodies\":[]}"));
}

/// The world as of the last frame (see [`state`]).
pub(crate) fn published() -> String {
    PUBLISHED.with(|text| text.borrow().clone())
}

/// `{ active, steps, bodies: [{ node, kind, held, position, rotation, speed }] }` for the page and the tests.
fn state(physics: &Physics) -> String {
    let bodies: Vec<serde_json::Value> = physics
        .world
        .as_ref()
        .map(|world| {
            physics
                .bodies
                .iter()
                .filter_map(|live| {
                    let pose = world.pose(live.id).ok()?;
                    let speed = world.velocity(live.id).ok()?;
                    Some(serde_json::json!({"node": live.node, "kind": format!("{:?}", live.kind), "held": live.held,
                        "position": pose.position, "rotation": pose.rotation, "speed": speed}))
                })
                .collect()
        })
        .unwrap_or_default();
    serde_json::json!({"active": physics.world.is_some(), "steps": physics.steps, "entities": physics.entities, "movable": physics.movable, "refused": physics.refused, "marks": physics.marks, "bodies": bodies,
        "walker": physics.walker.as_ref().and_then(|w| w.state).map(|s| serde_json::json!({"position": s.position, "grounded": s.grounded}))}).to_string()
}
