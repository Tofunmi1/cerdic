
/// One fixed step of the walker, steered by the keys.
fn walk_step(world: &mut World, walking: &mut Walking, rig: &ViewerRig) {
    let Some(input) = rig.walk_input() else {
        return;
    };
    let speed = walking.speed * if input.sprint { 2.0 } else { 1.0 };
    let wanted = [input.direction.x * speed, input.direction.z * speed];
    walking.state = world.walk(wanted, input.jump.then_some(walking.jump)).ok();
    if walking
        .state
        .is_some_and(|state| state.position[1] < -100.0)
        && let Ok(()) = world.set_walker(&WalkerDesc {
            position: walking.start,
            height: walking.height,
            radius: 0.3,
        })
    {
        walking.state = None;
    }
}

fn to_kind(kind: BodyKind) -> Kind {
    match kind {
        BodyKind::Static => Kind::Static,
        BodyKind::Dynamic => Kind::Dynamic,
        BodyKind::Kinematic => Kind::Kinematic,
    }
}

/// The transform that puts a part at body pose `(position, rotation)`.
fn transform_for(centre: Vec3, position: [f32; 3], rotation: [f32; 4]) -> Transform {
    let rotation = Quat::from_xyzw(rotation[0], rotation[1], rotation[2], rotation[3]).normalize();
    Transform {
        translation: Vec3::from_array(position) - rotation * centre,
        rotation,
        scale: Vec3::ONE,
    }
}

fn build(
    parts: &Query<(Entity, &Body, &mut Transform)>,
    gravity: [f32; 3],
    refused: &mut Vec<String>,
) -> Option<(World, Vec<Live>)> {
    let mut world = World::new(gravity);
    let mut by_node: BTreeMap<&str, (Arc<BodySpec>, Vec<Entity>, Transform)> = BTreeMap::new();
    for (entity, body, transform) in parts.iter() {
        by_node
            .entry(body.0.node.as_str())
            .or_insert_with(|| (Arc::clone(&body.0), Vec::new(), *transform))
            .1
            .push(entity);
    }
    let mut live = Vec::new();
    for (node, (spec, mut entities, transform)) in by_node {
        entities.sort();
        let at = transform.rotation * spec.centre + transform.translation;
        let desc = BodyDesc {
            kind: to_kind(spec.kind),
            shape: spec.shape.clone(),
            position: at.to_array(),
            rotation: transform.rotation.to_array(),
            density: spec.density,
            friction: spec.friction,
            restitution: spec.restitution,
            linear_damping: spec.linear_damping,
            angular_damping: spec.angular_damping,
        };
        // A body that cannot be built (a shape with no volume) is left out rather than failing the scene.
        let id = match world.add(&desc) {
            Ok(id) => id,
            Err(error) => {
                refused.push(format!("{node}: {error}"));
                continue;
            }
        };
        live.push(Live {
            node: node.to_owned(),
            id,
            kind: spec.kind,
            centre: spec.centre,
            held: false,
            history: VecDeque::new(),
            entities,
            spec: Arc::clone(&spec),
        });
    }
    connect(&mut world, &live, refused);
    (!live.is_empty()).then_some((world, live))
}

/// Adds every body's joints now that all bodies exist: a joint names the other part by its output name.
fn connect(world: &mut World, live: &[Live], refused: &mut Vec<String>) {
    let by_name: BTreeMap<&str, BodyId> = live
        .iter()
        .map(|body| (body.spec.name.as_str(), body.id))
        .collect();
    for body in live {
        for joint in &body.spec.joints {
            let other = if joint.to.is_empty() {
                Some(None)
            } else {
                by_name.get(joint.to.as_str()).map(|id| Some(*id))
            };
            let Some(other) = other else {
                refused.push(format!(
                    "{}: joint to \"{}\" has no body",
                    body.spec.name, joint.to
                ));
                continue;
            };
            if let Err(error) = world.add_joint(&joint_desc(body.id, other, joint)) {
                refused.push(format!(
                    "{}: joint to \"{}\": {error}",
                    body.spec.name, joint.to
                ));
            }
        }
    }
}

/// A graph joint as the physics world wants it (degrees to radians for hinges).
fn joint_desc(a: BodyId, b: Option<BodyId>, joint: &PhysJoint) -> JointDesc {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "authored joint numbers are small"
    )]
    let f = |v: f64| v as f32;
    let array = |v: [f64; 3]| [f(v[0]), f(v[1]), f(v[2])];
    let hinge = joint.kind == GraphJoint::Hinge;
    let angle = |v: f64| if hinge { f(v.to_radians()) } else { f(v) };
    JointDesc {
        a,
        b,
        kind: match joint.kind {
            GraphJoint::Fixed => JointKind::Fixed,
            GraphJoint::Hinge => JointKind::Hinge,
            GraphJoint::Slider => JointKind::Slider,
            GraphJoint::Ball => JointKind::Ball,
            GraphJoint::Spring => JointKind::Spring,
        },
        anchor: array(joint.anchor),
        anchor_b: array(joint.anchor_b),
        axis: array(joint.axis),
        limits: joint.limited.then(|| [angle(joint.low), angle(joint.high)]),
        stiffness: f(joint.stiffness),
        damping: f(joint.damping),
        motor_speed: angle(joint.motor_speed),
        motor_force: f(joint.motor_force),
        rest_length: f(joint.rest_length),
    }
}

/// A hand takes hold of a body (it turns kinematic and follows the hand's transform) or lets go of it (it turns
/// dynamic again and keeps the speed the hand had over the last few steps).
fn follow_hand(live: &mut Live, hand: Option<Transform>, held_now: bool, world: &mut World) {
    if held_now {
        if !live.held {
            live.held = true;
            live.history.clear();
            let _ = world.set_kind(live.id, Kind::Kinematic);
        }
        if let Some(transform) = hand {
            let at = transform.rotation * live.centre + transform.translation;
            let _ = world.set_pose(live.id, at.to_array(), transform.rotation.to_array());
            live.history.push_back(at);
            if live.history.len() > THROW_WINDOW {
                live.history.pop_front();
            }
        }
    } else if live.held {
        live.held = false;
        let _ = world.set_kind(live.id, to_kind(live.kind));
        let velocity = match (live.history.front(), live.history.back()) {
            #[expect(clippy::cast_precision_loss, reason = "a handful of steps")]
            (Some(first), Some(last)) if live.history.len() > 1 => {
                (*last - *first) / ((live.history.len() - 1) as f32 * STEP)
            }
            _ => Vec3::ZERO,
        };
        if live.kind == BodyKind::Dynamic {
            let _ = world.set_velocity(live.id, velocity.to_array(), [0.0; 3]);
        }
    }
}

/// Runs the world for the fixed steps taken this frame, poses the parts from it, and publishes the result.
pub(crate) fn run(
    mut behaviors: ResMut<Behaviors>,
    mut physics: ResMut<Physics>,
    mut parts: Query<(Entity, &Body, &mut Transform)>,
    mut rig: ResMut<ViewerRig>,
) {
    advance(&mut behaviors, &mut physics, &mut parts, &mut rig);
    PUBLISHED.with(|text| *text.borrow_mut() = state(&physics));
}
