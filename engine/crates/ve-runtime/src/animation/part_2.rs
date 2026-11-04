/// Poses every animated part from the clock, after the pose of the part it follows. A part with a behaviour is
/// posed from its own clip time instead. A child's own motion acts first (about its pivot, in the rest
/// frame), then its parent's pose carries it along.
pub(crate) fn animate(
    clock: Res<AnimationClock>,
    parts: Query<(Entity, &Animated, Option<&crate::behavior::Reactive>)>,
    mut transforms: Query<&mut Transform>,
    targets: Query<(Entity, &IkTarget)>,
    mut poses: ResMut<PartPoses>,
    time: Res<Time<Real>>,
    mix: Res<MachineMix>,
    placements: Res<InstancePlacement>,
    physics: Res<crate::physics::Physics>,
    mut springs: Local<HashMap<Entity, Option<ve_chain::Chain>>>,
    mut gaits: Local<HashMap<Entity, GaitState>>,
) {
    // Who hangs from whom, for the overlay masks of animation state machines.
    let parent_of: HashMap<&str, Option<&str>> = parts
        .iter()
        .map(|(_, animated, _)| (animated.name.as_str(), animated.parent.as_deref()))
        .collect();
    let mut locals: Vec<(Entity, &Animated, Mat4, f32, bool, Option<[f32; 3]>)> = parts
        .iter()
        .map(|(entity, animated, reactive)| {
            // A part with no behaviour plays its main clip only; a named clip waits for a behaviour (NaN: not playing).
            let time_of = |clip: &str| {
                reactive.map_or(
                    if clip.is_empty() {
                        clock.seconds
                    } else {
                        f32::NAN
                    },
                    |part| part.time_of(clip),
                )
            };
            // A part a state machine drives is posed from the blend of the clips the machine plays; any other from its own clips.
            let local = match mix.0.get(&animated.name) {
                Some(machine_pose) => layered_pose(&animated.tracks, machine_pose, &animated.name, &parent_of).to_matrix(),
                None => pose(&animated.tracks, &time_of).to_matrix(),
            };
            (
                entity,
                animated,
                local,
                glow(&animated.tracks, &time_of),
                shown(&animated.tracks, &time_of),
                tint(&animated.tracks, &time_of),
            )
        })
        .collect();
    let by_name: HashMap<&str, usize> = locals
        .iter()
        .enumerate()
        .map(|(i, (_, animated, ..))| (animated.name.as_str(), i))
        .collect();
    poses.0.clear();
    poses.1.clear();
    let no_ragdoll = std::collections::HashSet::new();
    let mut worlds = resolve_worlds(&locals, &by_name, &placements.0, &no_ragdoll);
    // Chains of bones reaching for a target: solve each, replace its bones' own poses, and resolve everything again.
    if !targets.is_empty() {
        for (entity, ik) in &targets {
            if let Some(settings) = ik.chain.secondary {
                let indices: Option<Vec<usize>> =
                    ik.chain.bones.iter().map(|name| by_name.get(name.as_str()).copied()).collect();
                let Some(indices) = indices else { continue };
                let parent_world = locals[indices[0]]
                    .1
                    .parent
                    .as_deref()
                    .and_then(|name| by_name.get(name))
                    .map_or(Mat4::IDENTITY, |&p| worlds[p]);
                let dt = if clock.playing { time.delta_secs() * clock.speed } else { 0.0 };
                let state = springs.entry(entity).or_default();
                if let Some(solved) = secondary_locals(&ik.chain, settings, parent_world, dt, state) {
                    for (&bone, local) in indices.iter().zip(solved) {
                        locals[bone].2 = local;
                    }
                }
                continue;
            }
            let goal = if let Some(settings) = &ik.chain.gait {
                // A walking foot: the gait puts the target where the foot goes, from the body's motion, and the target part follows.
                let body = by_name.get(settings.body.as_str()).map(|&b| worlds[b]);
                let dt = if clock.playing { time.delta_secs() * clock.speed } else { 0.0 };
                let placed = body.and_then(|body| gait_goal(&ik.chain, settings, ik.centre, body, dt, gaits.entry(entity).or_default()));
                if let Some(goal) = placed
                    && let Ok(mut transform) = transforms.get_mut(entity)
                {
                    transform.translation = goal - ik.centre;
                }
                placed.ok_or(())
            } else {
                transforms.get(entity).map(|t| t.translation + ik.centre).map_err(|_| ())
            };
            let indices: Option<Vec<usize>> = ik
                .chain
                .bones
                .iter()
                .map(|name| by_name.get(name.as_str()).copied())
                .collect();
            let (Ok(goal), Some(indices)) = (goal, indices) else {
                continue;
            };
            let root = indices[0];
            let parent_world = locals[root]
                .1
                .parent
                .as_deref()
                .and_then(|name| by_name.get(name))
                .map_or(Mat4::IDENTITY, |&p| worlds[p]);
            let solved = if let Some(look) = &ik.chain.aim {
                ve_modeling::ik::solve_aim_poses(
                    &ik.chain.segments,
                    parent_world.to_cols_array(),
                    &ve_modeling::ik_limb::AimGoal {
                        target: goal.as_dvec3(),
                        pole: Vec3::from_array(look.pole).as_dvec3(),
                        twist: f64::from(look.twist),
                        weight: f64::from(look.weight),
                    },
                    look.up,
                )
            } else if let Some(two) = &ik.chain.two_bone {
                ve_modeling::ik::solve_two_bone_poses(
                    &ik.chain.segments,
                    parent_world.to_cols_array(),
                    &ve_modeling::ik_limb::TwoBoneGoal {
                        target: goal.as_dvec3(),
                        pole: Vec3::from_array(two.pole).as_dvec3(),
                        soften: f64::from(two.soften),
                        twist: f64::from(two.twist),
                        weight: 1.0,
                    },
                )
            } else {
                ve_modeling::ik::solve_poses(
                    &ik.chain.segments,
                    parent_world.to_cols_array(),
                    goal.to_array(),
                    ik.chain.iterations,
                    ik.chain.limit_degrees,
                )
            };
            for (&bone, pose) in indices.iter().zip(&solved) {
                locals[bone].2 = Mat4::from_cols_array(pose);
            }
        }
        worlds = resolve_worlds(&locals, &by_name, &placements.0, &no_ragdoll);
    }
    // Ragdolls that have gone limp: their bones take the pose of their physics bodies, and what hangs from a bone follows it.
    if !physics.ragdoll_poses().is_empty() {
        let mut limp = std::collections::HashSet::new();
        for (name, pose) in physics.ragdoll_poses() {
            if let Some(&index) = by_name.get(name.as_str()) {
                locals[index].2 = *pose;
                limp.insert(index);
            }
        }
        worlds = resolve_worlds(&locals, &by_name, &placements.0, &limp);
    }
    for (start, (entity, animated, _, factor, visible, colour)) in locals.iter().enumerate() {
        let world = worlds[start];
        let target = Transform::from_matrix(world);
        if let Ok(mut transform) = transforms.get_mut(*entity)
            && *transform != target
        {
            *transform = target;
        }
        // A hidden part is dark and gives no light.
        poses.1.insert(animated.name.clone(), world.to_cols_array());
        poses.0.insert(
            animated.node.clone(),
            (
                world,
                if *visible { *factor } else { 0.0 },
                *visible,
                *colour,
            ),
        );
    }
}

/// Shows or hides parts whose tracks include `visible`.
pub(crate) fn apply_visibility(
    poses: Res<PartPoses>,
    parts: Query<(Entity, &Animated, Option<&InheritedVisibility>)>,
    mut commands: Commands,
) {
    for (entity, animated, current) in &parts {
        if !animated
            .tracks
            .iter()
            .any(|track| track.property == TrackProperty::Visible)
        {
            continue;
        }
        let Some((_, _, visible, _)) = poses.0.get(&animated.node) else {
            continue;
        };
        let want = if *visible {
            InheritedVisibility::VISIBLE
        } else {
            InheritedVisibility::HIDDEN
        };
        if current != Some(&want) {
            commands.entity(entity).insert(want);
        }
    }
}

/// One copy of a placed set whose prototype has tracks (ADR 0028 follow-up): where it is placed, which copy it is, and
/// the tracks every copy shares.
#[derive(Component, Clone)]
pub(crate) struct InstanceAnimated {
    pub(crate) tracks: std::sync::Arc<[Track]>,
    pub(crate) base: Mat4,
    pub(crate) index: u32,
}

/// Moves every copy of an animated placed set: its placement, then its tracks in its own frame, shifted by each track's
/// phase for this copy (so a field sways out of step). Only motion tracks apply to copies.
pub(crate) fn animate_instances(
    clock: Res<AnimationClock>,
    mut copies: Query<(&InstanceAnimated, &mut ve_render::VeInstanceTransform)>,
) {
    for (copy, mut placed) in &mut copies {
        let moved = pose_by(&copy.tracks, &|track| {
            clock.seconds + track.phase(copy.index)
        })
        .to_matrix();
        let matrix = copy.base * moved;
        if placed.0 != matrix {
            placed.0 = matrix;
        }
    }
}

/// Scales the emissive colour of glowing parts by their glow factor.
pub(crate) fn apply_glow(
    poses: Res<PartPoses>,
    mut parts: Query<(&Animated, &BaseEmissive, &mut VeMaterial)>,
) {
    for (animated, base, mut material) in &mut parts {
        let Some((_, factor, ..)) = poses.0.get(&animated.node) else {
            continue;
        };
        let base = LinearRgba::from(base.0);
        let lit = Color::LinearRgba(LinearRgba::new(
            base.red * factor,
            base.green * factor,
            base.blue * factor,
            base.alpha,
        ));
        if material.emissive != lit {
            material.emissive = lit;
        }
    }
}

/// Moves and dims the lights of animated parts with their parts.
pub(crate) fn animate_lights(
    poses: Res<PartPoses>,
    mut points: Query<
        (&Authored, &PartLight, &mut Transform, &mut VePointLight),
        Without<VeSpotLight>,
    >,
    mut spots: Query<
        (&Authored, &PartLight, &mut Transform, &mut VeSpotLight),
        Without<VePointLight>,
    >,
) {
    for (authored, base, mut transform, mut light) in &mut points {
        if let Some((world, factor, ..)) = poses.0.get(authored.node()) {
            transform.translation = world.transform_point3(base.position);
            light.intensity = base.intensity * factor;
        }
    }
    for (authored, base, mut transform, mut light) in &mut spots {
        if let Some((world, factor, ..)) = poses.0.get(authored.node()) {
            transform.translation = world.transform_point3(base.position);
            light.direction = world
                .transform_vector3(base.direction)
                .normalize_or(base.direction);
            light.intensity = base.intensity * factor;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ve_modeling::anim::{Interpolation, Key};

    fn track(property: TrackProperty, pivot: [f32; 3], end: [f32; 3]) -> Track {
        Track {
            property,
            keys: vec![
                Key {
                    time: 0.0,
                    value: if property == TrackProperty::Scale {
                        [1.0; 3]
                    } else {
                        [0.0; 3]
                    },
                },
                Key {
                    time: 1.0,
                    value: end,
                },
            ],
            interpolation: Interpolation::Linear,
            looped: false,
            pivot,
            clip: String::new(),
        }
    }

    #[test]
    fn a_door_turned_a_quarter_about_its_hinge_swings_its_far_edge() {
        // A point 1 m from the hinge along +X, turned 90 degrees about Y, ends 1 m along -Z.
        let door = [track(
            TrackProperty::Rotation,
            [-0.5, 0.0, 0.0],
            [0.0, 90.0, 0.0],
        )];
        let moved = pose(&door, &|_| 1.0)
            .to_matrix()
            .transform_point3(Vec3::new(0.5, 0.0, 0.0));
        assert!(
            (moved - Vec3::new(-0.5, 0.0, -1.0)).length() < 1e-5,
            "{moved}"
        );
        let hinge = pose(&door, &|_| 1.0)
            .to_matrix()
            .transform_point3(Vec3::new(-0.5, 0.0, 0.0));
        assert!(
            (hinge - Vec3::new(-0.5, 0.0, 0.0)).length() < 1e-5,
            "the hinge stays put: {hinge}"
        );
        assert!(
            pose(&door, &|_| 0.0)
                .to_matrix()
                .abs_diff_eq(Mat4::IDENTITY, 1e-6),
            "at rest at time zero"
        );
    }

    #[test]
    fn position_and_scale_combine_in_order() {
        let tracks = [
            track(TrackProperty::Scale, [0.0; 3], [2.0, 2.0, 2.0]),
            track(TrackProperty::Position, [0.0; 3], [0.0, 3.0, 0.0]),
        ];
        let moved = pose(&tracks, &|_| 1.0)
            .to_matrix()
            .transform_point3(Vec3::new(1.0, 0.0, 0.0));
        assert!(
            (moved - Vec3::new(2.0, 3.0, 0.0)).length() < 1e-5,
            "{moved}"
        );
    }

    #[test]
    fn each_clip_is_sampled_at_its_own_time() {
        let mut lift = track(TrackProperty::Position, [0.0; 3], [0.0, 3.0, 0.0]);
        lift.clip = "lift".to_owned();
        let tracks = [
            track(TrackProperty::Position, [0.0; 3], [2.0, 0.0, 0.0]),
            lift,
        ];
        let at = |main: f32, lift: f32| {
            pose(&tracks, &|clip| if clip == "lift" { lift } else { main })
                .to_matrix()
                .transform_point3(Vec3::ZERO)
        };
        assert!(
            (at(1.0, 0.0) - Vec3::new(2.0, 0.0, 0.0)).length() < 1e-5,
            "only the main clip has played"
        );
        assert!(
            (at(0.0, 1.0) - Vec3::new(0.0, 3.0, 0.0)).length() < 1e-5,
            "only the lift clip has played"
        );
        assert!(
            (at(1.0, 1.0) - Vec3::new(2.0, 3.0, 0.0)).length() < 1e-5,
            "both together combine"
        );
    }
}
