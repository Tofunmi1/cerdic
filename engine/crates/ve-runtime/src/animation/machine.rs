// Driving animation state machines (`anim.machine`): each machine decides which clips play and with what weights, and every part it drives
// is posed from the weighted blend of those clips. Included into `animation`, so it shares that module's imports and `Animated`.

/// An animation state machine on a part, and what it needs to run. The machine drives every animated part that shares the root of the
/// hierarchy this part belongs to (the part itself, the bones that follow it, the bones those follow ...).
#[derive(Component)]
pub(crate) struct MachineDrive {
    graph: std::sync::Arc<ve_animstate::StateMachine>,
    /// Built on the first frame, once the length of every clip is known from the tracks.
    machine: Option<ve_animstate::Machine>,
    /// Where the part stood last frame, for the speed it is moving at.
    last_position: Option<Vec3>,
    /// Parameters the host set itself; the runtime stops computing those.
    manual: std::collections::HashSet<String>,
}

impl MachineDrive {
    /// A machine with the same graph and a state of its own, for one game entity's copy of a model (ADR 0066).
    pub(crate) fn fresh(&self) -> Self {
        Self::new(std::sync::Arc::clone(&self.graph))
    }

    pub(crate) fn new(graph: std::sync::Arc<ve_animstate::StateMachine>) -> Self {
        Self {
            graph,
            machine: None,
            last_position: None,
            manual: std::collections::HashSet::new(),
        }
    }
}

/// This frame's pose of each machine, by the output name of every part it drives.
#[derive(Resource, Default)]
pub(crate) struct MachineMix(HashMap<String, std::sync::Arc<ve_animstate::Pose>>);

/// A request from the page or the game to a machine's parameters.
enum MachineRequest {
    Float(String, String, f32),
    Bool(String, String, bool),
    Trigger(String, String),
}

thread_local! {
    static MACHINE_REQUESTS: std::cell::RefCell<Vec<MachineRequest>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Queues a request for the machine on the part named `part` (empty: every machine). At most 1,024 wait at once.
fn queue_request(request: MachineRequest) {
    MACHINE_REQUESTS.with(|queue| {
        let mut queue = queue.borrow_mut();
        if queue.len() < 1024 {
            queue.push(request);
        }
    });
}

pub(crate) fn request_float(part: &str, name: &str, value: f32) {
    if value.is_finite() {
        queue_request(MachineRequest::Float(part.to_owned(), name.to_owned(), value));
    }
}

pub(crate) fn request_bool(part: &str, name: &str, value: bool) {
    queue_request(MachineRequest::Bool(part.to_owned(), name.to_owned(), value));
}

pub(crate) fn request_trigger(part: &str, name: &str) {
    queue_request(MachineRequest::Trigger(part.to_owned(), name.to_owned()));
}

/// The output name of the root of the hierarchy that `name` belongs to (the part itself when it follows nothing).
fn root_of<'a>(name: &'a str, parents: &'a HashMap<&'a str, Option<&'a str>>) -> &'a str {
    let mut at = name;
    for _ in 0..MAX_DEPTH {
        // A parent with no animation of its own (a carrier that only holds the machine) is still the root: it has no entry, so the walk stops.
        match parents.get(at).copied().flatten() {
            Some(parent) => at = parent,
            None => break,
        }
    }
    at
}

/// Steps every machine and publishes the pose each part it drives is blended from.
pub(crate) fn drive_machines(
    clock: Res<AnimationClock>,
    time: Res<Time<Real>>,
    animated: Query<&Animated>,
    mut machines: Query<(&Authored, &Transform, Option<&Animated>, &mut MachineDrive)>,
    mut mix: ResMut<MachineMix>,
) {
    mix.0.clear();
    let requests: Vec<MachineRequest> = MACHINE_REQUESTS.with(|queue| std::mem::take(&mut *queue.borrow_mut()));
    if machines.is_empty() {
        return;
    }
    let mut lengths: HashMap<String, f32> = HashMap::new();
    let mut parents: HashMap<&str, Option<&str>> = HashMap::new();
    for part in &animated {
        parents.insert(part.name.as_str(), part.parent.as_deref());
        for track in &part.tracks {
            let length = lengths.entry(track.clip.clone()).or_insert(0.0);
            *length = length.max(track.duration());
        }
    }
    let dt = if clock.playing { time.delta_secs() * clock.speed } else { 0.0 };
    for (authored, transform, own_tracks, mut drive) in &mut machines {
        let name = authored.name();
        let drive = &mut *drive;
        let machine = drive.machine.get_or_insert_with(|| ve_animstate::Machine::new((*drive.graph).clone(), &lengths));
        for request in &requests {
            let (part, ..) = match request {
                MachineRequest::Float(p, n, v) => (p, n, *v),
                MachineRequest::Bool(p, n, _) => (p, n, 0.0),
                MachineRequest::Trigger(p, n) => (p, n, 0.0),
            };
            if !part.is_empty() && part != name {
                continue;
            }
            match request {
                MachineRequest::Float(_, parameter, value) => {
                    drive.manual.insert(parameter.clone());
                    machine.set_float(parameter, *value);
                }
                MachineRequest::Bool(_, parameter, value) => machine.set_bool(parameter, *value),
                MachineRequest::Trigger(_, parameter) => machine.trigger(parameter),
            }
        }
        // A part that is only a carrier (no tracks of its own) is moved by the game or the page: its horizontal speed and its vertical
        // speed are what the machine's `speed` and `vy` read, unless the host sets them itself.
        let here = transform.translation;
        if own_tracks.is_none() && dt > 0.0 {
            if let Some(last) = drive.last_position {
                let moved = here - last;
                if !drive.manual.contains("speed") {
                    machine.set_float("speed", Vec3::new(moved.x, 0.0, moved.z).length() / dt);
                }
                if !drive.manual.contains("vy") {
                    machine.set_float("vy", moved.y / dt);
                }
            }
        }
        drive.last_position = Some(here);
        let pose = std::sync::Arc::new(machine.step(dt.min(ve_animstate::MAX_STEP)));
        // Clip events (a footstep, a hit frame) reach scripts as `anim.<name>` with the machine's part and the clip.
        for event in &pose.events {
            crate::world_api::push_event(&format!("anim.{}", event.name), name, serde_json::json!({ "clip": event.clip }));
            // And to the game, where a rule can say `on="host:anim.<name>"` (footsteps that add noise, a hit frame that damages).
            crate::world_api::queue_host_event(&event.name, name);
        }
        let root = root_of(name, &parents);
        for part in &animated {
            if root_of(part.name.as_str(), &parents) == root {
                mix.0.insert(part.name.clone(), std::sync::Arc::clone(&pose));
            }
        }
    }
}

/// The transform the tracks of a part give it when a machine plays `pose`: each clip in the pose is sampled at its own time, and the
/// results are blended by weight (a clip with no track on this part leaves it at rest for that share).
/// Whether the part called `part` is the part called `ancestor` or hangs below it (by `anim.parent`).
fn is_under(part: &str, ancestor: &str, parents: &HashMap<&str, Option<&str>>) -> bool {
    let mut at = part;
    for _ in 0..MAX_DEPTH {
        if at == ancestor {
            return true;
        }
        match parents.get(at).copied().flatten() {
            Some(parent) => at = parent,
            None => return false,
        }
    }
    false
}

fn trs_of(transform: Transform) -> ve_animstate::Trs {
    ve_animstate::Trs {
        translation: transform.translation.to_array(),
        rotation: transform.rotation.to_array(),
        scale: transform.scale.to_array(),
    }
}

fn transform_of(trs: ve_animstate::Trs) -> Transform {
    Transform {
        translation: Vec3::from(trs.translation),
        rotation: Quat::from_array(trs.rotation).normalize(),
        scale: Vec3::from(trs.scale),
    }
}

/// [`blended_pose`] with the machine's overlay layers (a wave over a walk) applied on top, to this part only if its mask names it or something it hangs from.
pub(crate) fn layered_pose(
    tracks: &[Track],
    pose: &ve_animstate::Pose,
    part: &str,
    parents: &HashMap<&str, Option<&str>>,
) -> Transform {
    let base = blended_pose(tracks, pose);
    if pose.overlays.is_empty() {
        return base;
    }
    let mut result = trs_of(base);
    for overlay in &pose.overlays {
        let acts = overlay.mask.is_empty() || overlay.mask.iter().any(|mask| is_under(part, mask, parents));
        // A part with no track in the overlay's clip is left as the machine has it (pulling it to rest would be a bug, not a blend).
        if !acts || !tracks.iter().any(|track| track.clip == overlay.clip) {
            continue;
        }
        let at = |seconds: f32| {
            trs_of(pose_by(tracks, &|track| if track.clip == overlay.clip { seconds } else { f32::NAN }))
        };
        result = if overlay.additive {
            ve_animstate::apply_additive(&result, &at(0.0), &at(overlay.time), overlay.weight)
        } else {
            ve_animstate::apply_override(&result, &at(overlay.time), overlay.weight)
        };
    }
    transform_of(result)
}

pub(crate) fn blended_pose(tracks: &[Track], pose: &ve_animstate::Pose) -> Transform {
    let items: Vec<(f32, ve_animstate::Trs)> = pose
        .layers
        .iter()
        .filter(|layer| layer.weight > 0.0)
        .map(|layer| {
            let sampled = pose_by(tracks, &|track| {
                if track.clip == layer.clip {
                    layer.time
                } else {
                    f32::NAN
                }
            });
            (
                layer.weight,
                ve_animstate::Trs {
                    translation: sampled.translation.to_array(),
                    rotation: sampled.rotation.to_array(),
                    scale: sampled.scale.to_array(),
                },
            )
        })
        .collect();
    ve_animstate::blend_trs(&items).map_or(Transform::IDENTITY, |trs| Transform {
        translation: Vec3::from(trs.translation),
        rotation: Quat::from_array(trs.rotation).normalize(),
        scale: Vec3::from(trs.scale),
    })
}
