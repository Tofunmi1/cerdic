
/// The mouse as a pointer. While the desktop viewer holds the pointer the ray is the crosshair at
/// the centre of the view (in mono or split-screen alike); otherwise it is the cursor, through a
/// plain (non-stereo) camera.
fn mouse_frame(
    cursor: &mut CursorState,
    buttons: &ButtonInput<MouseButton>,
    cameras: &Query<(&Camera, &GlobalTransform, Option<&StereoViews>), With<Camera3d>>,
    viewer: &ViewerRig,
) -> Option<PointerFrame> {
    let ray = if let Some((origin, direction)) = viewer.crosshair() {
        Ray::new(origin, direction)
    } else {
        let position = cursor.position?;
        let (camera, transform, _) = cameras.iter().find(|(_, _, stereo)| stereo.is_none())?;
        let world_ray = camera.viewport_to_world(transform, position).ok()?;
        Ray::new(world_ray.origin, *world_ray.direction)
    };
    Some(PointerFrame {
        id: PointerId(0),
        ray,
        grip: None,
        select: buttons.pressed(MouseButton::Left),
        squeeze: buttons.pressed(MouseButton::Right),
        reach: std::mem::take(&mut cursor.reach),
        select_grabs: true,
    })
}

/// One frame of interaction: gather pointers, run the machine, apply what it decided.
pub(crate) fn run_interaction(
    time: Res<Time<Real>>,
    rig: Res<XrRig>,
    xr_input: Res<XrInput>,
    buttons: Res<ButtonInput<MouseButton>>,
    hook: Res<SnapHook>,
    viewer: Res<ViewerRig>,
    mut cursor: ResMut<CursorState>,
    mut state: ResMut<InteractionState>,
    cameras: Query<(&Camera, &GlobalTransform, Option<&StereoViews>), With<Camera3d>>,
    labels: Query<&Label>,
    mut objects: Query<(
        Entity,
        &'static Interactable,
        &'static mut Transform,
        Option<&'static ve_render::VeInstanceTransform>,
    )>,
    mut lasers: Query<(&Laser, &mut Transform), Without<Interactable>>,
) {
    let dt = time.delta_secs();
    // A controller that lost tracking (no poses) is treated as gone, not as one that let go: the
    // machine releases what it held and drops its hover exactly once, and it cannot resume a grab
    // by reappearing with the squeeze still down at a different place.
    let hands: Vec<(Handedness, PointerFrame)> = xr_input
        .controllers
        .iter()
        .map(|controller| {
            (
                controller.handedness,
                controller_pointer(controller, rig.0, dt),
            )
        })
        .filter(|(_, frame)| frame.ray.is_some() || frame.grip.is_some())
        .collect();
    let mut frames: Vec<PointerFrame> = mouse_frame(&mut cursor, &buttons, &cameras, &viewer)
        .into_iter()
        .collect();
    frames.extend(hands.iter().map(|(_, frame)| *frame));

    let readonly = objects.as_readonly();
    let world = SceneWorld {
        objects: &readonly,
        hook: &hook,
    };
    let output = state.machine.update(&frames, &world);
    // Each laser reaches as far as what its controller points at.
    let reach: Vec<(Handedness, Ray, f32)> = hands
        .iter()
        .filter_map(|(hand, frame)| {
            let ray = frame.ray?;
            let length = world
                .pick(&ray, LASER_REST * 2.0)
                .map_or(LASER_REST, |hit| hit.distance);
            Some((*hand, ray, length))
        })
        .collect();

    for event in &output.events {
        let line = describe(event, &labels);
        state.note(line);
        match event {
            Event::SelectEnd {
                target,
                click: true,
                ..
            } => {
                if let Some(entity) = entity_of(*target) {
                    state.clicked.push(entity);
                }
            }
            Event::GrabStart { target, .. } => {
                if let Some(entity) = entity_of(*target) {
                    state.grabs.push((entity, true));
                }
            }
            Event::GrabEnd { target, .. } => {
                if let Some(entity) = entity_of(*target) {
                    state.grabs.push((entity, false));
                }
            }
            _ => {}
        }
    }
    state.hovered = (0..=3_u32)
        .filter_map(|id| state.machine.hover_of(PointerId(id)))
        .filter_map(entity_of)
        .collect();

    for (target, placement) in &output.placements {
        if let Some(entity) = entity_of(*target)
            && let Ok((_, _, mut transform, _)) = objects.get_mut(entity)
        {
            // Keep any non-uniform scale: only the uniform factor changes.
            let old = transform.scale.x;
            if old.abs() > f32::EPSILON {
                transform.scale *= placement.scale / old;
            }
            transform.translation = placement.translation;
            transform.rotation = placement.rotation;
        }
    }
    for (Laser(hand), mut transform) in &mut lasers {
        *transform = reach.iter().find(|(h, _, _)| h == hand).map_or_else(
            || Transform::from_scale(Vec3::ZERO),
            |(_, ray, length)| laser_transform(ray, *length),
        );
    }
}

/// Places a unit-length laser bar along `ray`, `length` metres long.
fn laser_transform(ray: &Ray, length: f32) -> Transform {
    Transform {
        translation: ray.at(length * 0.5),
        rotation: Quat::from_rotation_arc(Vec3::NEG_Z, ray.direction),
        scale: Vec3::new(1.0, 1.0, length),
    }
}
