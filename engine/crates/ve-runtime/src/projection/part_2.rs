pub(crate) fn update(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut ambient: ResMut<VeAmbientLight>,
    mut sky_background: ResMut<ve_render::SkyBackground>,
    mut textures: ResMut<ve_render::MaterialTextures>,
    baseline: Res<crate::scene::SceneAmbient>,
    old: Query<(Entity, &Mesh3d), With<Authored>>,
    gallery: Query<(Entity, &Mesh3d), With<GalleryObject>>,
    old_lights: Query<Entity, With<lights::AuthoredLight>>,
) {
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        for handle in state.discarded.drain(..) {
            meshes.remove(handle.id());
        }
        let Some(mut candidate) = state.candidate.take() else {
            return;
        };
        if !staging::current(&candidate.token) {
            state
                .discarded
                .extend(candidate.uploaded.into_iter().map(|item| item.handle));
            return;
        }
        if let Some(chunk) = candidate.queue.pop_front() {
            let began = now();
            let bytes = size_of_val(chunk.data.as_slice());
            candidate.queued_bytes -= bytes;
            let skinned =
                chunk.region == 0 && chunk.group == 0 && candidate.skins.contains_key(&chunk.node);
            let corners = skinned.then(|| candidate.corners.entry(chunk.node.clone()).or_default());
            candidate.uploaded.push(upload(chunk, &mut meshes, corners));
            state.last_ms = now() - began;
            state.max_ms = state.max_ms.max(state.last_ms);
            state.upload_frames += 1;
            state.max_frame_bytes = state.max_frame_bytes.max(bytes);
        }
        if candidate.finished
            && candidate.queue.is_empty()
            && candidate
                .uploaded
                .iter()
                .all(|item| mesh_ready(item.handle.id()))
            && staging::accept(&candidate.token)
        {
            for (entity, mesh) in old.iter().chain(gallery.iter()) {
                commands.entity(entity).despawn();
                meshes.remove(mesh.0.id());
            }
            for entity in &old_lights {
                commands.entity(entity).despawn();
            }
            lights::spawn(&mut commands, &candidate.lights, &candidate.token);
            state.light_count = candidate.lights.len();
            // An authored environment replaces the scene ambient; a document without one restores
            // what the built-in scene set at startup.
            *ambient = candidate
                .environment
                .as_ref()
                .map_or(baseline.0, environment::ambient);
            *sky_background = candidate
                .environment
                .as_ref()
                .and_then(|scene| scene.sky.as_ref())
                .map(|sky| ve_render::SkyBackground {
                    enabled: true,
                    horizon: sky.horizon_color,
                    zenith: sky.zenith_color,
                    ground: sky.ground_color,
                    sun_direction: sky.sun_direction,
                    sun_intensity: sky.sun_intensity,
                    turbidity: sky.turbidity,
                    clouds: sky.clouds,
                    cloud_cover: sky.cloud_cover,
                    seed: sky.seed,
                })
                .unwrap_or_default();
            state.environment.clone_from(&candidate.environment);
            state.parts.clear();
            state.instance_sets.clear();
            candidate.count_instance_sets(&mut state.instance_sets);
            crate::selection::validate_instances(&state.instance_sets);
            let materials = std::mem::take(&mut candidate.materials);
            // The new scene's maps replace the old ones; each region keeps the layers it was given.
            let layers = assign_layers(&mut textures, std::mem::take(&mut candidate.maps));
            let mut animated = 0_usize;
            let uploaded = std::mem::take(&mut candidate.uploaded);
            let bodies = candidate.body_specs(&uploaded);
            for item in uploaded {
                state
                    .parts
                    .push((item.node.clone(), candidate.token.revision));
                let key = (item.node.clone(), item.region);
                let mut surface = materials.get(&key).copied().unwrap_or(VeMaterial {
                    base_color: Color::srgb(0.68, 0.73, 0.87),
                    roughness: 0.5,
                    ..VeMaterial::default()
                });
                surface.map_layers = layers.get(&key).copied().unwrap_or([-1; 4]);
                let placements = candidate.placements.get(&item.group);
                let mut motion = candidate.motion(&item.node, &item.name);
                motion.body = bodies.get(&item.node);
                animated += usize::from(motion.moves() && placements.is_none());
                placements::spawn(
                    &mut commands,
                    item,
                    &candidate.token,
                    surface,
                    placements,
                    &motion,
                );
            }
            state.animated_parts = animated;
            state.committed = Some(candidate.token);
            return;
        }
        state.candidate = Some(candidate);
    });
}

pub(crate) fn status() -> String {
    STATE.with(|cell| {
        let state = cell.borrow();
        json!({"committed":state.committed,
        "queued_bytes":state.candidate.as_ref().map_or(0, |c| c.queued_bytes),
        "staging_meshes":state.candidate.as_ref().map_or(0, |c| c.uploaded.len()),
        "live_meshes":state.parts.len(), "live_lights":state.light_count, "animated_parts":state.animated_parts, "selection_map":state.parts,
        "live_environment":state.environment,
        "instance_sets":state.instance_sets,
        "upload_frames":state.upload_frames,"max_frame_bytes":state.max_frame_bytes,
        "last_upload_ms":state.last_ms,"max_upload_ms":state.max_ms})
        .to_string()
    })
}

pub(crate) fn selections(query: &Query<(Entity, &Authored)>) -> serde_json::Value {
    query.iter().take(128).map(|(entity, item)| json!({"entity":entity.to_bits(),"node":item.node,"revision":item.revision})).collect()
}
