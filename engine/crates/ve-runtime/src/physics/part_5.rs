/// Parts that carry a body and are not the hidden prototype of a model (each copy of a character carries its own).
type Prototypes<'w, 's> = Query<'w, 's, (), bevy_ecs::query::With<crate::game_models::ModelPrototype>>;

thread_local! {
    /// The pose of every moving body as a matrix (what the part's transform does to its authored geometry), by the part's output name, for
    /// simulation programs that read a body like a bone.
    static POSES: std::cell::RefCell<std::collections::HashMap<String, [f32; 16]>> = std::cell::RefCell::new(std::collections::HashMap::new());
    /// Speed changes simulation programs ask for, by part name, applied at the next physics step.
    static PUSHES: std::cell::RefCell<Vec<(String, [f32; 3])>> = const { std::cell::RefCell::new(Vec::new()) };
}

impl Body {
    /// The body of one more copy of a character: its own node and part name (so copies never share a body), and joints that name the
    /// copy's own parts. The scene-wide settings stay with the original.
    pub(crate) fn copy_for(&self, rename: &dyn Fn(&str) -> String) -> Body {
        let spec = &self.0;
        let joints = spec
            .joints
            .iter()
            .map(|joint| {
                let mut joint = joint.clone();
                if !joint.to.is_empty() {
                    joint.to = rename(&joint.to);
                }
                joint
            })
            .collect();
        Body(Arc::new(BodySpec {
            node: rename(&spec.node),
            name: rename(&spec.name),
            joints,
            world: None,
            kind: spec.kind,
            shape: spec.shape.clone(),
            centre: spec.centre,
            density: spec.density,
            friction: spec.friction,
            restitution: spec.restitution,
            linear_damping: spec.linear_damping,
            angular_damping: spec.angular_damping,
        }))
    }
}

/// Most pushes held between two physics steps.
const MAX_QUEUED_PUSHES: usize = 4096;

/// The pose matrix of the moving body of the part called `name` as of the last frame.
pub(crate) fn body_matrix(name: &str) -> Option<[f32; 16]> {
    POSES.with(|poses| poses.borrow().get(name).copied())
}

/// Asks for the body of the part called `name` to change speed by `change` metres per second at the next step.
pub(crate) fn queue_push(name: &str, change: [f32; 3]) {
    PUSHES.with(|queue| {
        let mut queue = queue.borrow_mut();
        if queue.len() < MAX_QUEUED_PUSHES {
            queue.push((name.to_owned(), change));
        }
    });
}

/// Forgets every push still waiting. A program keeps stepping while the scene is only being edited, and with no physics running nothing
/// drains the queue, so without this the whole backlog (up to the cap) would hit the bodies in one step when play starts.
pub(crate) fn clear_pushes() {
    PUSHES.with(|queue| queue.borrow_mut().clear());
}

fn apply_pushes(world: &mut World, bodies: &[Live]) {
    let queued = PUSHES.with(|queue| std::mem::take(&mut *queue.borrow_mut()));
    for (name, change) in queued {
        if let Some(live) = bodies.iter().find(|live| live.spec.name == name && live.kind == BodyKind::Dynamic) {
            let _ = world.push(live.id, change);
        }
    }
}

/// A kinematic body goes where its part has been put (by an animation, a parent): the world moves it and pushes what is in its way.
fn follow_part(live: &Live, transform: Transform, world: &mut World) {
    let at = transform.rotation * live.centre + transform.translation;
    let _ = world.set_pose(live.id, at.to_array(), transform.rotation.to_array());
}

fn add_body(world: &mut World, node: &str, spec: &Arc<BodySpec>, entities: Vec<Entity>, transform: Transform, refused: &mut Vec<String>) -> Option<Live> {
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
        layers: ve_physics::Layers::default(),
        sensor: false,
    };
    match world.add(&desc) {
        Ok(id) => Some(Live {
            node: node.to_owned(),
            id,
            kind: spec.kind,
            centre: spec.centre,
            held: false,
            history: VecDeque::new(),
            entities,
            spec: Arc::clone(spec),
        }),
        Err(error) => {
            refused.push(format!("{node}: {error}"));
            None
        }
    }
}

/// Keeps the running world in step with the scene: a body whose parts are all gone (a character that left the game) is removed with its
/// joints; a part that appeared since the world was built (a character that was spawned) gets its body and joints. A hidden model prototype
/// never has a body: its copies do.
fn sync_bodies(physics: &mut Physics, parts: &Query<(Entity, &Body, &mut Transform)>, prototypes: &Prototypes) {
    let Physics { world, bodies, refused, .. } = physics;
    let Some(world) = world.as_mut() else {
        return;
    };
    let wanted: HashSet<Entity> = parts.iter().filter(|(entity, ..)| !prototypes.contains(*entity)).map(|(entity, ..)| entity).collect();
    bodies.retain_mut(|live| {
        live.entities.retain(|entity| wanted.contains(entity));
        if live.entities.is_empty() {
            let _ = world.remove(live.id);
            false
        } else {
            true
        }
    });
    let known: HashSet<Entity> = bodies.iter().flat_map(|live| live.entities.iter().copied()).collect();
    let mut fresh: BTreeMap<String, (Arc<BodySpec>, Vec<Entity>, Transform)> = BTreeMap::new();
    for (entity, body, transform) in parts.iter() {
        if wanted.contains(&entity) && !known.contains(&entity) {
            fresh.entry(body.0.node.clone()).or_insert_with(|| (Arc::clone(&body.0), Vec::new(), *transform)).1.push(entity);
        }
    }
    let first_new = bodies.len();
    for (node, (spec, mut entities, transform)) in fresh {
        entities.sort();
        if let Some(live) = bodies.iter_mut().find(|live| live.node == node) {
            live.entities.extend(entities);
        } else if let Some(live) = add_body(world, &node, &spec, entities, transform, refused) {
            bodies.push(live);
        }
    }
    if bodies.len() > first_new {
        connect_from(world, bodies, first_new, refused);
    }
}

/// Publishes where every moving body is, for simulation programs that read bodies like bones.
fn publish_poses(bodies: &[Live], parts: &Query<(Entity, &Body, &mut Transform)>) {
    POSES.with(|poses| {
        let mut poses = poses.borrow_mut();
        poses.clear();
        for live in bodies.iter().filter(|live| live.kind != BodyKind::Static) {
            if let Some((_, _, transform)) = live.entities.first().and_then(|entity| parts.get(*entity).ok()) {
                poses.insert(live.spec.name.clone(), transform.to_matrix().to_cols_array());
            }
        }
    });
}
