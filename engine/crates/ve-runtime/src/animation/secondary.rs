/// Secondary motion (`rig.secondary`): a chain of bone parts follows its root by springs (`ve-chain`). Each frame the chain is stepped from the
/// pose of the part the first bone hangs from, and every bone gets the local pose that puts its head on the chain's point and turns its rest
/// direction to the direction of the next point. Bone `i`'s own tracks are replaced. Returns one local matrix per bone (root first), or `None`
/// when the chain cannot be built (its state is then dropped).
fn secondary_locals(
    chain: &ve_modeling::rig::IkChain,
    settings: ve_modeling::rig::Secondary,
    parent_world: Mat4,
    dt: f32,
    state: &mut Option<ve_chain::Chain>,
) -> Option<Vec<Mat4>> {
    let segments = &chain.segments;
    let first = segments.first()?;
    let (_, parent_rotation, _) = parent_world.to_scale_rotation_translation();
    let head0 = Vec3::from_array(first.head);
    let root = ve_chain::RootPose {
        position: parent_world.transform_point3(head0).as_dvec3().to_array(),
        rotation: parent_rotation.as_dquat().to_array(),
    };
    if state.is_none() {
        let spec = ve_chain::ChainSpec {
            rest: segments
                .iter()
                .map(|s| {
                    let d = Vec3::from_array(s.tail) - Vec3::from_array(s.head);
                    d.as_dvec3().to_array()
                })
                .collect(),
            stiffness: f64::from(settings.stiffness),
            damping: f64::from(settings.damping),
            gravity: settings.gravity.map(f64::from),
            iterations: 8,
        };
        *state = ve_chain::Chain::new(spec, root).ok();
    }
    let live = state.as_mut()?;
    live.step(root, f64::from(dt));
    let points: Vec<Vec3> = live
        .points()
        .iter()
        .map(|p| Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32))
        .collect();
    let mut previous_world = parent_world;
    let mut locals = Vec::with_capacity(segments.len());
    for (i, segment) in segments.iter().enumerate() {
        let head = Vec3::from_array(segment.head);
        let rest_direction = (Vec3::from_array(segment.tail) - head).normalize_or_zero();
        let now = (points.get(i + 1)? - points.get(i)?).normalize_or_zero();
        let carried = parent_rotation * rest_direction;
        let turn = Quat::from_rotation_arc(carried, now) * parent_rotation;
        let world = Mat4::from_translation(*points.get(i)?) * Mat4::from_quat(turn) * Mat4::from_translation(-head);
        locals.push(previous_world.inverse() * world);
        previous_world = world;
    }
    Some(locals)
}

/// A walking foot's state between frames: its gait, and where its body was, for the body's velocity.
#[derive(Default)]
pub(crate) struct GaitState {
    gait: Option<ve_gait::Gait>,
    last_body: Option<Vec3>,
}

/// Where a `rig.gait` foot target goes this frame: the gait of one leg, from its body's world pose. The leg's hip is the head of its first
/// bone, its rest foot the target's own authored place; the body's velocity is its movement since the last frame.
fn gait_goal(
    chain: &ve_modeling::rig::IkChain,
    settings: &ve_modeling::rig::GaitSettings,
    centre: Vec3,
    body: Mat4,
    dt: f32,
    state: &mut GaitState,
) -> Option<Vec3> {
    let first = chain.segments.first()?;
    let (_, rotation, position) = body.to_scale_rotation_translation();
    let (yaw, _, _) = rotation.to_euler(EulerRot::YXZ);
    let velocity = match (state.last_body, dt > 1.0e-5) {
        (Some(last), true) => ((position - last) / dt).as_dvec3().to_array(),
        _ => [0.0; 3],
    };
    state.last_body = Some(position);
    if state.gait.is_none() {
        let length: f32 = chain
            .segments
            .iter()
            .map(|s| (Vec3::from_array(s.tail) - Vec3::from_array(s.head)).length())
            .sum();
        let ground = f64::from(settings.ground);
        let spec = ve_gait::GaitSpec {
            legs: vec![ve_gait::Leg {
                hip: first.head.map(f64::from),
                rest_foot: [f64::from(centre.x), ground, f64::from(centre.z)],
                reach: f64::from(if settings.reach > 0.0 { settings.reach } else { length }),
                phase: f64::from(settings.phase),
            }],
            stride_length: f64::from(settings.stride_length),
            step_height: f64::from(settings.step_height),
            duty: f64::from(settings.duty),
        };
        state.gait = ve_gait::Gait::new(spec).ok();
    }
    let ground = f64::from(settings.ground);
    let gait = state.gait.as_mut()?;
    gait.step(
        ve_gait::BodyPose { position: position.as_dvec3().to_array(), yaw: f64::from(yaw) },
        velocity,
        f64::from(dt),
        &move |_x, _z| ground,
    );
    let foot = gait.feet().first()?.position;
    Some(Vec3::new(foot[0] as f32, foot[1] as f32 + (centre.y - settings.ground), foot[2] as f32))
}
