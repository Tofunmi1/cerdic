
/// Applies a pose the page asked to jump to (see `set_look_at`), then its zoom, orbit and framing.
fn jump_to_pose(rig: &mut ViewerRig, input: &Input) {
    if let Some((position, yaw, pitch)) = input.pose {
        (rig.position, rig.yaw, rig.pitch) = (position, yaw, pitch);
    }
    if rig.driven.is_some() {
        // a game camera owns the view: the wheel and orbit drags adjust that camera (zoom, turn, tilt) instead of moving the free viewer
        let (zoom, orbit) = viewer_nav::take_adjustments();
        rig.adjust_game_camera(zoom, orbit);
        return;
    }
    viewer_nav::apply(
        &mut rig.position,
        &mut rig.yaw,
        &mut rig.pitch,
        vertical_fov(input),
    );
}

/// Turns the viewer by the mouse movement and moves it by the wanted motion, for `dt` seconds.
fn step(rig: &mut ViewerRig, input: &Input, dt: f32) {
    if rig.driven == Some(false) {
        return; // a fixed-angle game camera holds the view: no mouse look, no flying
    }
    rig.yaw -= input.look.x * SENSITIVITY;
    rig.pitch = (rig.pitch - input.look.y * SENSITIVITY).clamp(-FRAC_PI_2 + 0.01, FRAC_PI_2 - 0.01);
    if rig.walking {
        // Physics moves a walking viewer; the mouse still turns it.
        return;
    }
    let wanted =
        rig.rotation() * Vec3::new(input.motion.x, 0.0, -input.motion.z) + Vec3::Y * input.motion.y;
    let boost = if input.sprint { SPRINT } else { 1.0 };
    rig.position += wanted.clamp_length_max(1.0) * input.speed * boost * dt.min(MAX_STEP);
}

/// Integrates pending navigation and publishes the camera snapshot used by viewport handles.
fn move_and_publish(rig: &mut ViewerRig, input: &Input, dt: f32) -> (Quat, f32) {
    jump_to_pose(rig, input);
    step(rig, input, dt);
    let rotation = rig.rotation();
    let vertical = vertical_fov(input);
    CAMERA.with(|camera| {
        camera.set(Some(CameraView {
            position: rig.position,
            rotation,
            vertical_fov: vertical,
        }));
    });
    (rotation, vertical)
}

/// The vertical field of view to use, in radians: the page's, or a comfortable default.
fn vertical_fov(input: &Input) -> f32 {
    if input.fov_deg > 0.0 {
        input.fov_deg.to_radians()
    } else {
        std::f32::consts::FRAC_PI_3
    }
}

/// Moves and turns the viewer, and applies it to the scene's camera.
pub(crate) fn update_viewer(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut rig: ResMut<ViewerRig>,
    mut lens: ResMut<LensWarp>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<
        (
            Entity,
            &mut Transform,
            Option<&mut MockHead>,
            Option<&mut Projection>,
        ),
        With<Camera3d>,
    >,
) {
    let input = with_input(|input| {
        let snapshot = *input;
        input.look = Vec2::ZERO;
        input.reset = false;
        if snapshot.enabled {
            input.pose = None;
        }
        snapshot
    });
    rig.locked = input.enabled && input.locked;
    let presenting = session_state() == SessionState::Presenting;
    let warp = LensWarp {
        mode: wanted_lens(&input, input.stereo || rig.native_stereo, presenting),
        source_half_fov_tan: (vertical_fov(&input) * 0.5).tan(),
    };
    if *lens != warp {
        *lens = warp;
    }
    if !input.enabled || presenting {
        return;
    }
    let Some((entity, mut transform, head, projection)) = cameras.iter_mut().next() else {
        return;
    };
    if rig.home.is_none() {
        // Start from wherever the scene put its camera.
        let (position, rotation) = head.as_ref().map_or_else(
            || (transform.translation, transform.rotation),
            |h| (h.pose.position, h.pose.orientation),
        );
        let (yaw, pitch, _) = rotation.to_euler(EulerRot::YXZ);
        rig.home = Some((position, yaw, pitch));
        rig.native_stereo = head.is_some();
        (rig.position, rig.yaw, rig.pitch) = (position, yaw, pitch);
    }
    if input.reset
        && let Some((position, yaw, pitch)) = rig.home
    {
        (rig.position, rig.yaw, rig.pitch) = (position, yaw, pitch);
    }

    let (rotation, vertical) = move_and_publish(&mut rig, &input, time.delta_secs());
    if input.stereo || rig.native_stereo {
        let size = windows.single().map_or((2, 1), |w| {
            (
                w.resolution.physical_width(),
                w.resolution.physical_height(),
            )
        });
        let pose = Pose {
            position: rig.position,
            orientation: rotation,
        };
        let new_head =
            crate::mock_hmd::viewer_head(pose, head.as_deref(), input.fov_deg, vertical, size);
        if let Some(mut existing) = head {
            *existing = new_head;
        } else {
            // A scene without a mock headset: split the view, and leave the real headset's camera
            // marker off (it would strip the eyes every frame).
            commands
                .entity(entity)
                .insert((new_head, stereo_views(&new_head, size)))
                .remove::<XrCamera>();
        }
    } else {
        if head.is_some() {
            commands.entity(entity).remove::<(MockHead, StereoViews)>();
        }
        transform.translation = rig.position;
        transform.rotation = rotation;
        if input.fov_deg > 0.0
            && let Some(mut projection) = projection
        {
            *projection = Projection::Perspective(PerspectiveProjection {
                fov: vertical,
                near: NEAR,
                ..PerspectiveProjection::default()
            });
        }
    }
}
