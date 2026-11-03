// Methods of the runtime world API that are not component reads and writes (ADR 0065): the animation parameters, and visible shapes a
// script can spawn (a bullet, a piece of debris, a marker). Included into `world_api`, so it shares that module's imports.

/// Most entities `world.spawn_entity` may have alive at once (a script that spawns without despawning is stopped at this).
const MAX_API_ENTITIES: usize = 2_000;
/// Meshes kept for reuse (one per distinct shape and size).
const MAX_SHAPE_MESHES: usize = 64;

thread_local! {
    static SHAPE_MESHES: RefCell<HashMap<String, bevy_asset::Handle<bevy_mesh::Mesh>>> = RefCell::new(HashMap::new());
}

/// `animation.set_float {part, name, value}`, `animation.set_bool {part, name, value}` and `animation.trigger {part, name}`: drive an
/// animation state machine from a script or a model. `part` is the part the machine is on (empty: every machine).
fn animation_method(method: &str, params: &Value) -> Result<Value, String> {
    let part = params.get("part").and_then(Value::as_str).unwrap_or("");
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .filter(|n| !n.is_empty())
        .ok_or("an animation parameter needs a name")?;
    match method {
        "animation.set_float" => {
            let value = params
                .get("value")
                .and_then(Value::as_f64)
                .ok_or("animation.set_float needs a number value")?;
            #[expect(
                clippy::cast_possible_truncation,
                reason = "an animation parameter, well inside f32"
            )]
            crate::animation::request_float(part, name, value as f32);
        }
        "animation.set_bool" => {
            let value = params
                .get("value")
                .and_then(Value::as_bool)
                .ok_or("animation.set_bool needs true or false")?;
            crate::animation::request_bool(part, name, value);
        }
        _ => crate::animation::request_trigger(part, name),
    }
    Ok(json!({ "queued": method, "part": part, "name": name }))
}

/// `world.bounds {entity}`: where a part is and how big, in world coordinates: `{center: [x, y, z], size: [x, y, z]}`. A part's `Transform`
/// is only an offset from where it was built, so this is how a script finds the place to spawn debris or aim at.
fn bounds(world: &mut World, params: &Value) -> Result<Value, String> {
    let entity = entity_of(
        world,
        params.get("entity").ok_or("world.bounds needs an entity")?,
    )?;
    let aabb = world
        .get::<bevy_camera::primitives::Aabb>(entity)
        .copied()
        .ok_or("this entity has no bounds yet (it has no mesh, or has not been drawn once)")?;
    let global = world
        .get::<bevy_transform::components::GlobalTransform>(entity)
        .copied()
        .unwrap_or_default();
    let centre = global.transform_point(aabb.center.into());
    let scale = global.to_scale_rotation_translation().0;
    let half = bevy_math::Vec3::from(aabb.half_extents) * 2.0 * scale.abs();
    Ok(json!({ "center": [centre.x, centre.y, centre.z], "size": [half.x, half.y, half.z] }))
}

/// The mask a physics query uses: the groups to look at (default every group).
fn query_mask(params: &Value) -> u32 {
    params
        .get("mask")
        .and_then(Value::as_u64)
        .map_or(u32::MAX, |mask| u32::try_from(mask).unwrap_or(u32::MAX))
}

fn found_json(found: &crate::physics::Found) -> Value {
    json!({ "part": found.part, "node": found.node })
}

/// `physics.raycast {origin, direction, max_distance?, mask?}`: the first physics body a ray meets in play mode:
/// `{hit: false}` or `{hit: true, part, node, point, normal, distance}`. For aiming, line of sight and hitscan weapons.
fn physics_raycast(world: &mut World, params: &Value) -> Result<Value, String> {
    let origin = number_list(params.get("origin"), 3, "the ray origin")?;
    let direction = number_list(params.get("direction"), 3, "the ray direction")?;
    #[expect(clippy::cast_possible_truncation, reason = "scene numbers are well inside f32")]
    let (origin, direction) = (
        [origin[0] as f32, origin[1] as f32, origin[2] as f32],
        [direction[0] as f32, direction[1] as f32, direction[2] as f32],
    );
    #[expect(clippy::cast_possible_truncation, reason = "a distance, well inside f32")]
    let max = params.get("max_distance").and_then(Value::as_f64).map_or(100.0, |d| d as f32);
    let mut physics = world
        .get_resource_mut::<crate::physics::Physics>()
        .ok_or("physics is not available in this scene")?;
    Ok(match physics.ray(origin, direction, max, query_mask(params))? {
        Some((found, hit)) => json!({
            "hit": true, "part": found.part, "node": found.node,
            "point": hit.point, "normal": hit.normal, "distance": hit.distance
        }),
        None => json!({ "hit": false }),
    })
}

/// `physics.overlap {center, radius, mask?}`: the parts that have a physics body inside a sphere in play mode: `{parts: [{part, node}]}`.
fn physics_overlap(world: &mut World, params: &Value) -> Result<Value, String> {
    let centre = number_list(params.get("center"), 3, "the sphere centre")?;
    #[expect(clippy::cast_possible_truncation, reason = "scene numbers are well inside f32")]
    let (centre, radius) = (
        [centre[0] as f32, centre[1] as f32, centre[2] as f32],
        params.get("radius").and_then(Value::as_f64).map_or(1.0, |r| r as f32),
    );
    let mut physics = world
        .get_resource_mut::<crate::physics::Physics>()
        .ok_or("physics is not available in this scene")?;
    let found = physics.overlap(centre, radius, query_mask(params))?;
    let mut parts: Vec<Value> = found.iter().map(found_json).collect();
    parts.dedup();
    Ok(json!({ "parts": parts }))
}

/// `physics.ragdoll {name, impulse?}`: makes the character with the `phys.ragdoll` called `name` go limp from the pose it is in, with an optional
/// shove `impulse` [x, y, z] (kilogram metres per second) on its root bone. Play mode only. `{bones: n}`.
fn physics_ragdoll(world: &mut World, params: &Value) -> Result<Value, String> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .filter(|n| !n.is_empty())
        .ok_or("physics.ragdoll needs the name of a phys.ragdoll")?;
    #[expect(clippy::cast_possible_truncation, reason = "an impulse, well inside f32")]
    let impulse = match params.get("impulse") {
        None | Some(Value::Null) => [0.0; 3],
        some => {
            let v = number_list(some, 3, "the impulse")?;
            [v[0] as f32, v[1] as f32, v[2] as f32]
        }
    };
    let mut query = world.query::<&crate::physics::RagdollRig>();
    let spec = query
        .iter(world)
        .find(|rig| rig.0.name == name)
        .map(|rig| std::sync::Arc::clone(&rig.0))
        .ok_or_else(|| format!("no phys.ragdoll is called {name:?}; the scene's ragdolls are made with phys.ragdoll@1(name=...)"))?;
    world.resource_scope::<crate::physics::Physics, _>(|world, mut physics| {
        let poses = world.get_resource::<crate::animation::PartPoses>().ok_or("no poses yet")?;
        let bones = physics.start_ragdoll(&spec, poses, impulse)?;
        Ok(json!({ "ragdoll": name, "bones": bones }))
    })
}

/// `fx.burst {name, position, scale}`: a one-shot burst of particles where something happens. `name` is `explode` (fireball, sparks and
/// smoke; the default), `sparks`, `smoke`, `dust`, `fire`, `splash`, `magic`, `steam` or `embers`; `scale` 0.1 to 10.
fn fx_burst(params: &Value) -> Result<Value, String> {
    let position = number_list(params.get("position"), 3, "a burst position")?;
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("explode");
    #[expect(clippy::cast_possible_truncation, reason = "a scale, well inside f32")]
    let scale = params
        .get("scale")
        .and_then(Value::as_f64)
        .map_or(1.0, |s| s as f32);
    if crate::fx_bursts::request(name, [position[0], position[1], position[2]], scale) {
        Ok(json!({ "burst": name, "position": position }))
    } else {
        Err(format!(
            "unknown burst {name:?}; use explode, sparks, smoke, dust, fire, splash, magic, steam or embers"
        ))
    }
}

/// One point of the game's ground field: where it is, how high, which way it slopes and what surface a rolling thing is on there.
fn ground_point(
    field: &ve_game::GroundField,
    surfaces: &[ve_game::SurfaceDef],
    x: f64,
    z: f64,
) -> Value {
    let id = usize::from(field.surface(x, z));
    let (name, decel, bounce) = match id.checked_sub(1).and_then(|i| surfaces.get(i)) {
        Some(surface) => (surface.name.as_str(), surface.decel, surface.bounce),
        None => ("default", 1.0, 1.0),
    };
    json!({
        "x": x, "z": z,
        "height": field.height(x, z),
        "normal": field.normal(x, z),
        "surface": id, "surface_name": name, "decel": decel, "bounce": bounce,
    })
}

/// `ground.sample {x, z}` and `ground.profile {from: [x, z], to: [x, z], samples}`: read the ground a rolling thing runs on (height, slope,
/// surface and its deceleration), at a point or along a line (at most 400 samples). `height` is null where there is no ground (a void).
fn ground_query(world: &mut World, method: &str, params: &Value) -> Result<Value, String> {
    let runtime = world
        .get_resource::<crate::game::GameRuntime>()
        .ok_or("no game is loaded")?;
    let game = runtime.world.as_ref().ok_or("no game is loaded")?;
    let field = game.ground().ok_or(
        "the game has no ground field yet (it declares no terrain, or the parts are not built)",
    )?;
    let surfaces = game.surface_declarations();
    if method == "ground.sample" {
        let at = number_list(
            Some(&json!([params.get("x"), params.get("z")])),
            2,
            "x and z",
        )?;
        return Ok(ground_point(field, surfaces, at[0], at[1]));
    }
    let from = number_list(params.get("from"), 2, "from")?;
    let to = number_list(params.get("to"), 2, "to")?;
    let count = params
        .get("samples")
        .and_then(Value::as_u64)
        .map_or(50, |n| usize::try_from(n).unwrap_or(50))
        .clamp(2, 400);
    let length = (to[0] - from[0]).hypot(to[1] - from[1]);
    let points: Vec<Value> = (0..count)
        .map(|i| {
            #[expect(clippy::cast_precision_loss, reason = "at most 400 samples")]
            let t = i as f64 / (count - 1) as f64;
            let mut point = ground_point(
                field,
                surfaces,
                from[0] + (to[0] - from[0]) * t,
                from[1] + (to[1] - from[1]) * t,
            );
            point["s"] = json!(length * t);
            point
        })
        .collect();
    Ok(json!({ "length": length, "points": points }))
}

fn number_list(value: Option<&Value>, count: usize, what: &str) -> Result<Vec<f64>, String> {
    let list: Vec<f64> = value
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default();
    if list.len() == count && list.iter().all(|v| v.is_finite()) {
        Ok(list)
    } else {
        Err(format!("{what} must be {count} numbers"))
    }
}

/// A visible primitive for `world.spawn_entity {shape: {...}}`:
/// `{kind: "box", size: [x, y, z]}`, `{kind: "sphere", radius}` or `{kind: "cylinder", radius, height}`, with `color` [r, g, b] (0 to 1),
/// optional `emissive` [r, g, b], `metallic`, `roughness`, `opacity`. Returns the mesh and the surface; meshes are reused by shape and size.
fn build_shape(
    world: &mut World,
    shape: &Value,
) -> Result<(bevy_mesh::Mesh3d, ve_render::VeMaterial), String> {
    let kind = shape
        .get("kind")
        .and_then(Value::as_str)
        .ok_or("a shape needs a kind: box, sphere or cylinder")?;
    let in_range = |v: f64| (0.001..=100.0).contains(&v);
    let (key, solid) = match kind {
        "box" => {
            let size = number_list(shape.get("size"), 3, "a box size")?;
            if !size.iter().all(|v| in_range(*v)) {
                return Err("a box size is three lengths between 0.001 and 100 metres".to_owned());
            }
            (
                format!("box:{size:?}"),
                ve_kernel::Solid::cube([size[0], size[1], size[2]], true),
            )
        }
        "sphere" => {
            let radius = shape
                .get("radius")
                .and_then(Value::as_f64)
                .filter(|r| in_range(*r))
                .ok_or("a sphere needs a radius between 0.001 and 100 metres")?;
            (
                format!("sphere:{radius}"),
                ve_kernel::Solid::sphere(radius, 16),
            )
        }
        "cylinder" => {
            let radius = shape
                .get("radius")
                .and_then(Value::as_f64)
                .filter(|r| in_range(*r))
                .ok_or("a cylinder needs a radius between 0.001 and 100 metres")?;
            let height = shape
                .get("height")
                .and_then(Value::as_f64)
                .filter(|h| in_range(*h))
                .ok_or("a cylinder needs a height between 0.001 and 100 metres")?;
            (
                format!("cylinder:{radius}:{height}"),
                ve_kernel::Solid::cylinder(height, radius, radius, 20, true),
            )
        }
        other => {
            return Err(format!(
                "unknown shape kind {other:?}; use box, sphere or cylinder"
            ));
        }
    };
    let cached = SHAPE_MESHES.with(|meshes| meshes.borrow().get(&key).cloned());
    let handle = if let Some(handle) = cached {
        handle
    } else {
        let mesh = crate::scene::render_mesh(&solid.map_err(|e| e.to_string())?, false)
            .map_err(|e| e.to_string())?;
        let handle = world
            .resource_mut::<bevy_asset::Assets<bevy_mesh::Mesh>>()
            .add(mesh);
        SHAPE_MESHES.with(|meshes| {
            let mut meshes = meshes.borrow_mut();
            if meshes.len() >= MAX_SHAPE_MESHES {
                meshes.clear();
            }
            meshes.insert(key, handle.clone());
        });
        handle
    };
    let colour = |name: &str, default: [f64; 3]| -> Result<bevy_color::Color, String> {
        let c = if shape.get(name).is_some() {
            number_list(shape.get(name), 3, &format!("{name} is [r, g, b]; it"))?
        } else {
            default.to_vec()
        };
        #[expect(clippy::cast_possible_truncation, reason = "a colour component")]
        Ok(bevy_color::Color::srgb(
            c[0] as f32,
            c[1] as f32,
            c[2] as f32,
        ))
    };
    let scalar = |name: &str, default: f32| -> f32 {
        #[expect(clippy::cast_possible_truncation, reason = "a surface parameter")]
        shape
            .get(name)
            .and_then(Value::as_f64)
            .map_or(default, |v| v as f32)
    };
    let material = ve_render::VeMaterial {
        base_color: colour("color", [0.8, 0.8, 0.8])?,
        emissive: colour("emissive", [0.0, 0.0, 0.0])?,
        metallic: scalar("metallic", 0.0).clamp(0.0, 1.0),
        roughness: scalar("roughness", 0.6).clamp(0.0, 1.0),
        opacity: scalar("opacity", 1.0).clamp(0.0, 1.0),
        ..ve_render::VeMaterial::default()
    };
    Ok((bevy_mesh::Mesh3d(handle), material))
}

/// Interactive water forcing and displacement queries.
fn water_method(world: &mut World, method: &str, params: &Value) -> Result<Value, String> {
    crate::water_sim::method(world, method, params)
}
