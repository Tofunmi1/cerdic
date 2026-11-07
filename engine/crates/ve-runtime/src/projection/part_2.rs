pub(crate) fn update(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut ambient: ResMut<VeAmbientLight>,
    mut sky_background: ResMut<ve_render::SkyBackground>,
    mut textures: ResMut<ve_render::MaterialTextures>,
    mut probes: ResMut<ve_render::ProbeVolume>,
    mut wind: ResMut<ve_render::WindField>,
    baseline: Res<crate::scene::SceneAmbient>,
    old: Query<(Entity, &Mesh3d), With<Authored>>,
    old_sets: Query<(Entity, &ve_render::VeInstanceSet), With<Authored>>,
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
        // Several chunks a frame while they are cheap, up to a few milliseconds and a byte cap: one chunk a frame made a scene of
        // hundreds of chunks last hundreds of frames, which passed a minute on a machine drawing the scene at a few frames a second.
        let frame_began = now();
        let mut frame_bytes = 0;
        let mut drained = false;
        while let Some(chunk) = candidate.queue.pop_front() {
            drained = true;
            let began = now();
            let bytes = size_of_val(chunk.data.as_slice());
            frame_bytes += bytes;
            candidate.queued_bytes -= bytes;
            // A live cloth keeps its corner bookkeeping like a skin does (its chunks find their vertices by corner offset).
            let skinned = chunk.region == 0
                && chunk.group == 0
                && (candidate.skins.contains_key(&chunk.node)
                    || candidate.cloths.contains_key(&chunk.node)
                    || candidate.fluids.contains_key(&chunk.node)
                    || candidate.sims.contains_key(&chunk.node));
            let corners = skinned.then(|| candidate.corners.entry(chunk.node.clone()).or_default());
            let skin = skinned
                .then(|| candidate.skins.get(&chunk.node))
                .flatten()
                .cloned();
            // A skinned part with procedural relief keeps its rest position in the vertex colour for the pixel shader (see `detail.wesl`).
            let relief = skinned
                && candidate
                    .materials
                    .get(&(chunk.node.clone(), chunk.region))
                    .is_some_and(|material| material.finish.relief > 0.0);
            candidate
                .uploaded
                .push(upload(chunk, &mut meshes, corners, skin.as_deref(), relief));
            state.last_ms = now() - began;
            state.max_ms = state.max_ms.max(state.last_ms);
            if now() - frame_began > FRAME_UPLOAD_MS || frame_bytes >= FRAME_UPLOAD_BYTES {
                break;
            }
        }
        if drained {
            state.upload_frames += 1;
            state.max_frame_bytes = state.max_frame_bytes.max(frame_bytes);
        }
        if candidate.finished
            && candidate.queue.is_empty()
            && candidate
                .uploaded
                .iter()
                .all(|item| mesh_ready(item.handle.id()))
            && (candidate.draft || staging::accept(&candidate.token))
        {
            for (entity, mesh) in old.iter().chain(gallery.iter()) {
                commands.entity(entity).despawn();
                meshes.remove(mesh.0.id());
            }
            for (entity, set) in &old_sets {
                commands.entity(entity).despawn();
                for level in &set.meshes {
                    meshes.remove(level.mesh.id());
                }
            }
            // A draft leaves the lights, sky, wind and light probes of the scene on screen alone: it carries none of them.
            if !candidate.draft {
                for entity in &old_lights {
                    commands.entity(entity).despawn();
                }
                lights::spawn(&mut commands, &candidate.lights, &candidate.token);
                state.light_count = candidate.lights.len();
            }
            if !candidate.draft {
            // An authored environment replaces the scene ambient; a document without one restores
            // what the built-in scene set at startup.
            *ambient = candidate
                .environment
                .as_ref()
                .map_or(baseline.0, environment::ambient);
            *sky_background = candidate
                .environment
                .as_ref()
                .map(|scene| {
                    // The document's distance haze, light shafts and camera effects, which the renderer had never read. An
                    // environment without a sky still carries them (the sky pass stays off).
                    let carried = ve_render::SkyBackground {
                        haze: scene.haze.density,
                        haze_color: scene.haze.color,
                        haze_height: scene.haze.height,
                        shafts: scene.haze.shafts,
                        focus_distance: scene.haze.focus_distance,
                        aperture: scene.haze.aperture,
                        motion_blur: scene.haze.motion_blur,
                        contact_shadows: scene.haze.contact_shadows,
                        ..ve_render::SkyBackground::default()
                    };
                    scene
                        .sky
                        .as_ref()
                        .map_or(carried, |sky| ve_render::SkyBackground {
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
                            ..carried
                        })
                })
                .unwrap_or_default();
            state.environment.clone_from(&candidate.environment);
            // The wind an environment authors, or the default breeze; the clock keeps running.
            let blowing = candidate
                .environment
                .as_ref()
                .map_or_else(ve_modeling::Wind::default, |scene| scene.wind);
            *wind = ve_render::WindField {
                time: wind.time,
                ..ve_render::WindField::from_degrees(blowing.direction, blowing.strength)
            };
            // Light probes are traced from the geometry collected below; until they finish the scene keeps the sky ambient.
            gi::clear(&mut probes);
            }
            let mut collector = if candidate.draft {
                None
            } else {
                gi::Collector::new(candidate.environment.as_ref(), gi::sun(&candidate.lights))
            };
            state.parts.clear();
            state.instance_sets.clear();
            candidate.count_instance_sets(&mut state.instance_sets);
            crate::selection::validate_instances(&state.instance_sets);
            let materials = std::mem::take(&mut candidate.materials);
            // The new scene's maps replace the old ones; each region keeps the layers it was given.
            let layers = assign_layers(&mut textures, std::mem::take(&mut candidate.maps));
            maps::remember_layers(&layers);
            state.texture_maps = textures.len();
            state.texture_bytes = textures.bytes();
            let mut animated = 0_usize;
            let uploaded = std::mem::take(&mut candidate.uploaded);
            let mut survey = survey::Collector::new(&uploaded, &candidate.placements, &meshes);
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
                survey.add(&item, placements, &meshes);
                if let Some(collector) = collector.as_mut() {
                    collector.add(
                        &item,
                        &surface,
                        placements.map(|p| p.matrices.as_slice()),
                        &meshes,
                    );
                }
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
            survey.publish();
            if let Some(collector) = collector {
                collector.start();
            }
            state.animated_parts = animated;
            if !candidate.draft {
                state.committed = Some(candidate.token);
            }
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
        "live_environment":state.environment, "gi_progress":gi::progress(),
        "texture_maps":state.texture_maps, "texture_bytes":state.texture_bytes,
        "instance_sets":state.instance_sets,
        "upload_frames":state.upload_frames,"max_frame_bytes":state.max_frame_bytes,
        "last_upload_ms":state.last_ms,"max_upload_ms":state.max_ms})
        .to_string()
    })
}

pub(crate) fn selections(query: &Query<(Entity, &Authored)>) -> serde_json::Value {
    query.iter().take(128).map(|(entity, item)| json!({"entity":entity.to_bits(),"node":item.node,"revision":item.revision})).collect()
}
