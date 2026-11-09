//! Live game loading, fixed stepping, status publication, and authored-part drawings.
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, VecDeque},
};

use bevy_camera::visibility::Visibility;
use bevy_ecs::{
    component::Component,
    entity::Entity,
    query::With,
    resource::Resource,
    system::{Commands, Query, Res, ResMut},
};
use bevy_math::Vec3;
use bevy_mesh::Mesh3d;
use bevy_time::{Real, Time};
use bevy_transform::components::Transform;
use serde_json::{Value, json};
use ve_game::{EffectKind, GameDef, Inputs, Status, World};
use ve_render::{StereoViews, VeMaterial};
use ve_xr_web::{XrCamera, XrInput, XrRig};

use crate::{
    game_hands,
    game_view::{display_huds, place_viewer_at_start, push_player, render_position, viewer_feet},
    projection::Authored,
    viewer::ViewerRig,
};

const STEP: f64 = 1.0 / 60.0;
const MAX_STEPS: usize = 4;
const MAX_FRAME_DELTA: f64 = STEP * 4.0;
const EFFECT_LIMIT: usize = 32;

thread_local! {
    static REQUEST: RefCell<Option<LoadRequest>> = const { RefCell::new(None) };
    static STATUS: RefCell<String> = RefCell::new(empty_status());
    /// The most recent numbered effects, each with where it happened, for hosts that play them (audio, particles).
    static EFFECTS: RefCell<VecDeque<Value>> = const { RefCell::new(VecDeque::new()) };
    static EFFECT_SEQ: RefCell<u64> = const { RefCell::new(0) };
    /// A save asked for by the page, and the save text the game wrote in answer (empty until it has).
    static SAVE_REQUESTED: RefCell<bool> = const { RefCell::new(false) };
    static SAVE_OUT: RefCell<String> = const { RefCell::new(String::new()) };
    /// A save the page wants loaded, and what happened when the game tried ("ok" or the reason).
    static RESTORE: RefCell<Option<ve_game::Snapshot>> = const { RefCell::new(None) };
    static RESTORE_RESULT: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Asks for a save of the running game; the game answers on its next frame (see [`take_save`]).
pub(crate) fn request_save() {
    SAVE_OUT.with(|out| out.borrow_mut().clear());
    SAVE_REQUESTED.with(|flag| *flag.borrow_mut() = true);
}

/// The save the game wrote since [`request_save`], as JSON text, or an empty string while it has not (or when no game is running).
pub(crate) fn take_save() -> String {
    SAVE_OUT.with(|out| out.borrow().clone())
}

/// Queues a save game for loading into the running game. Returns the reason when the text is not a save game; whether it fits the
/// running game is known on the next frame (see [`restore_result`]).
pub(crate) fn request_restore(text: &str) -> String {
    match ve_game::Snapshot::from_json(text) {
        Ok(snapshot) => {
            RESTORE_RESULT.with(|result| result.borrow_mut().clear());
            RESTORE.with(|slot| *slot.borrow_mut() = Some(snapshot));
            String::new()
        }
        Err(error) => error.to_string(),
    }
}

/// How the last queued restore went: empty while pending, `ok`, or the reason it was refused.
pub(crate) fn restore_result() -> String {
    RESTORE_RESULT.with(|result| result.borrow().clone())
}

/// Effects numbered after `after`, as a JSON array (`[]` when none): each `{seq, kind, name, target, position}`.
pub(crate) fn effects_since(after: u64) -> String {
    EFFECTS.with(|list| {
        let newer: Vec<Value> = list
            .borrow()
            .iter()
            .filter(|effect| effect["seq"].as_u64().is_some_and(|seq| seq > after))
            .cloned()
            .collect();
        Value::Array(newer).to_string()
    })
}

fn push_effect(
    kind: EffectKind,
    name: String,
    target: Option<u32>,
    position: Option<[f64; 3]>,
) -> Value {
    let seq = EFFECT_SEQ.with(|seq| {
        let mut seq = seq.borrow_mut();
        *seq += 1;
        *seq
    });
    let mut value = effect_json(kind, name, target);
    value["seq"] = json!(seq);
    value["position"] = json!(position);
    EFFECTS.with(|list| {
        let mut list = list.borrow_mut();
        list.push_back(value.clone());
        while list.len() > EFFECT_LIMIT * 2 {
            list.pop_front();
        }
    });
    value
}

enum LoadRequest {
    Clear,
    Load(GameDef),
}

/// Prior visibility, saved while an authored mesh is acting as a game-part prototype.
#[derive(Component)]
pub(crate) struct PrototypeVisibility(Option<Visibility>);

/// A mesh entity drawing one live game entity from an authored prototype.
#[derive(Component)]
pub(crate) struct GameDrawing;

thread_local! {
    static UI_ASPECT: std::cell::Cell<f64> = const { std::cell::Cell::new(16.0 / 9.0) };
    static UI_FRAME: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

thread_local! {
    static GAME_AXES: std::cell::RefCell<std::collections::BTreeMap<String, f64>> = const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
    static GAME_PRESSES: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
    /// The loaded game's declared inputs and their device bindings, as JSON (see `GameDef::bindings_json`).
    static GAME_BINDINGS: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

thread_local! {
    /// What the page calls each action on the device in use, with a version that grows each time it changes (see [`game_ui_labels`]).
    static UI_LABELS: std::cell::RefCell<(u64, std::collections::BTreeMap<String, String>)> = const { std::cell::RefCell::new((0, std::collections::BTreeMap::new())) };
}

/// Tells the game what the player's device calls each action, as JSON `{"jump": "Space", "move": "W A S D"}`, for `{key:ACTION}` in the
/// game's UI texts. The page sends it when the player's device changes. Text that is not such an object is ignored.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn game_ui_labels(json: &str) {
    if let Ok(labels) = serde_json::from_str::<std::collections::BTreeMap<String, String>>(json) {
        UI_LABELS.with(|slot| {
            let mut slot = slot.borrow_mut();
            *slot = (slot.0 + 1, labels);
        });
    }
}

/// The loaded game's declared inputs with their keys, gamepad controls and labels, as JSON
/// `[{"name","kind","axes","keys","pad","label"}]` (empty text when no game is loaded). The page builds its bindings from this.
#[wasm_bindgen::prelude::wasm_bindgen]
#[must_use]
pub fn game_input_map() -> String {
    GAME_BINDINGS.with(|table| table.borrow().clone())
}

/// Sets a named input axis the game's scripts read with `input.axis(name)`: a held key or stick, usually -1 to 1. The page sets it as it
/// changes and it stays until set again; a non-finite value clears it.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn game_input_axis(name: &str, value: f64) {
    GAME_AXES.with(|axes| {
        if value.is_finite() && value != 0.0 {
            axes.borrow_mut().insert(name.to_owned(), value);
        } else {
            axes.borrow_mut().remove(name);
        }
    });
}

/// A button went down: scripts see `input.pressed(name)` true for the next step only.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn game_input_press(name: &str) {
    GAME_PRESSES.with(|presses| {
        let mut presses = presses.borrow_mut();
        if presses.len() < 32 && !presses.iter().any(|existing| existing == name) {
            presses.push(name.to_owned());
        }
    });
}

/// The game's UI as laid out at the last step, `{"aspect": a, "items": [...]}` with rect and path items in screen heights (y down): the
/// page draws them over the view. `aspect` is the screen's width over its height; the layout follows it from the next step on.
#[wasm_bindgen::prelude::wasm_bindgen]
#[must_use]
pub fn game_ui_frame(aspect: f64) -> String {
    if aspect.is_finite() && aspect > 0.2 {
        UI_ASPECT.with(|slot| slot.set(aspect));
    }
    UI_FRAME.with(|slot| slot.borrow().clone())
}

/// Mutable simulation and drawings for the currently loaded game.
#[derive(Resource, Default)]
pub(crate) struct GameRuntime {
    pub(super) world: Option<World>,
    accumulator: f64,
    steps: u64,
    effects: VecDeque<Value>,
    drawings: HashMap<(u32, Entity), Entity>,
    /// How many authored parts the game binds and keeps hidden as prototypes.
    hidden_prototypes: usize,
    /// How many skinned parts the copies of game models carry (a character drawn as a model).
    model_skins: usize,
    /// The spawn the headset's tracking origin still has to be moved to (set when a game loads, used once a session presents).
    xr_start: Option<Vec3>,
    /// Where the hands were at the end of the last frame, so the next frame's steps move them smoothly to where they are now.
    last_hands: Vec<ve_game::HandInput>,
    /// The game's cameras and which one is active (none when the game declares no camera).
    cameras: Option<ve_game::CameraRig>,
    /// The version of the page's action labels the world has been given.
    labels_seen: u64,
}

/// Queues a parsed game for the Bevy app and returns a parser diagnostic on failure.
pub(crate) fn load(text: &str) -> String {
    let requested = if text.is_empty() {
        GAME_BINDINGS.with(|table| table.borrow_mut().clear());
        LoadRequest::Clear
    } else {
        match GameDef::parse(text) {
            Ok(definition) => {
                GAME_BINDINGS.with(|table| *table.borrow_mut() = definition.bindings_json());
                LoadRequest::Load(definition)
            }
            Err(diagnostics) => {
                return diagnostics
                    .into_iter()
                    .map(|item| {
                        format!(
                            "{}:{}: {} ({})",
                            item.line, item.column, item.message, item.suggestion
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
            }
        }
    };
    REQUEST.with(|request| *request.borrow_mut() = Some(requested));
    String::new()
}

/// Snapshot of the live game's externally visible state.
pub(crate) fn status() -> String {
    let stored = STATUS.with(|status| status.borrow().clone());
    // Panel centres are only known to the HUD system, so they are added when the status is read.
    let Ok(mut value) = serde_json::from_str::<Value>(&stored) else {
        return stored;
    };
    value["hud"]["centers"] = json!(crate::hud::centers());
    value.to_string()
}

/// Advances the simulation and keeps prototype visibility and game drawings in sync.
#[expect(
    clippy::too_many_lines,
    reason = "one Bevy system keeps simulation state and its paired drawings synchronized"
)]
pub(crate) fn update(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut rig: ResMut<ViewerRig>,
    mut runtime: ResMut<GameRuntime>,
    sources: Query<(
        Entity,
        &Authored,
        &Mesh3d,
        &VeMaterial,
        Option<&Visibility>,
        Option<&PrototypeVisibility>,
    )>,
    mut drawing_transforms: Query<&mut Transform, With<GameDrawing>>,
    models: Res<crate::game_models::ModelInstances>,
    xr_cameras: Query<&StereoViews, With<XrCamera>>,
    mut xr_rig: Option<ResMut<XrRig>>,
    xr_input: Option<Res<XrInput>>,
) {
    if let Some(request) = REQUEST.with(|cell| cell.borrow_mut().take()) {
        runtime.cameras = match &request {
            LoadRequest::Load(def) if !def.cameras.is_empty() => Some(ve_game::CameraRig::new(def.cameras.clone())),
            _ => None,
        };
        runtime.world = match request {
            LoadRequest::Clear => None,
            LoadRequest::Load(def) => {
                let world = World::new(&def, 1);
                place_viewer_at_start(&world);
                Some(world)
            }
        };
        // In a headset the tracking origin is moved to the spawn the first frame a session is presenting.
        runtime.xr_start = runtime
            .world
            .as_ref()
            .and_then(World::viewer_start)
            .map(render_position);
        runtime.accumulator = 0.0;
        runtime.labels_seen = 0;
        runtime.steps = 0;
        runtime.effects.clear();
        EFFECTS.with(|list| list.borrow_mut().clear());
        for entity in runtime.drawings.drain().map(|(_, entity)| entity) {
            commands.entity(entity).despawn();
        }
    }

    // A save to load, and a save to write: both between steps, on the world as the last frame left it.
    if let Some(snapshot) = RESTORE.with(|slot| slot.borrow_mut().take()) {
        let outcome = match runtime.world.as_mut() {
            Some(world) => world
                .restore(&snapshot)
                .map_or_else(|error| error.to_string(), |()| "ok".to_owned()),
            None => "no game is running to load the save into".to_owned(),
        };
        RESTORE_RESULT.with(|result| *result.borrow_mut() = outcome);
        runtime.accumulator = 0.0;
    }
    if SAVE_REQUESTED.with(|flag| std::mem::take(&mut *flag.borrow_mut())) {
        let text = runtime
            .world
            .as_ref()
            .map(|world| world.snapshot().to_json())
            .unwrap_or_default();
        SAVE_OUT.with(|out| *out.borrow_mut() = text);
    }

    // the page's wording for the actions goes to the world whenever it changes and to every game that loads
    let (version, labels) = UI_LABELS.with(|slot| slot.borrow().clone());
    if runtime.labels_seen != version
        && let Some(world) = runtime.world.as_mut()
    {
        world.set_ui_labels(labels);
        runtime.labels_seen = version;
    }
    let GameRuntime {
        world,
        accumulator,
        steps,
        effects,
        drawings,
        hidden_prototypes,
        model_skins,
        xr_start,
        last_hands,
        cameras,
        labels_seen: _,
    } = &mut *runtime;
    if cameras.is_none() || world.is_none() {
        // no game camera: the free viewer has the view (and its mouse, wheel and hotkeys)
        rig.release();
    }
    if let Some(world) = world.as_mut()
        && (world.status() == Status::Playing || world.has_flow())
    {
        *accumulator += f64::from(time.delta_secs()).min(MAX_FRAME_DELTA);
        let feet = viewer_feet(&rig, &xr_cameras, xr_rig.as_deref_mut(), xr_start);
        let (hands_now, rotations_now) = match (xr_input.as_deref(), xr_rig.as_deref()) {
            (Some(controllers), Some(rig)) => (
                game_hands::hands(controllers, rig),
                game_hands::rotations(controllers, rig),
            ),
            _ => (
                game_hands::hands(&XrInput::default(), &XrRig::default()),
                game_hands::rotations(&XrInput::default(), &XrRig::default()),
            ),
        };
        let base = Inputs {
            viewer: Some(feet.to_array().map(f64::from)),
            hand_rotations: rotations_now,
            ..Inputs::default()
        };
        // Animation events since the last frame become `host:anim.<name>` events in the first step of this one.
        let animation_events: Vec<ve_game::HostEvent> = crate::world_api::take_host_events()
            .into_iter()
            .map(|(name, entity)| ve_game::HostEvent {
                name: format!("anim.{name}"),
                a: entity,
                b: None,
            })
            .collect();
        // A frame may run several fixed steps; the hands move the whole way from where they were to where they are across them, so a swing
        // has a speed in every step (a club head's velocity is what a shot is made of).
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a small count of whole steps"
        )]
        let due = ((*accumulator / STEP).floor() as usize).clamp(1, MAX_STEPS);
        let mut count = 0;
        while *accumulator >= STEP && count < MAX_STEPS {
            #[expect(clippy::cast_precision_loss, reason = "a count of at most four steps")]
            let blend = (count + 1) as f64 / due as f64;
            let mut input = base.clone();
            if count == 0 {
                input.host.clone_from(&animation_events);
                input.pressed =
                    GAME_PRESSES.with(|presses| std::mem::take(&mut *presses.borrow_mut()));
            }
            input.axes = GAME_AXES.with(|axes| axes.borrow().clone());
            // Where the player looks, for a controller that moves relative to the camera (compass yaw: 0 looks along +z).
            // the direction the camera is actually drawn looking (a fixed-angle camera's own angle, not the free viewer's stale look)
            let forward = rig.rotation() * Vec3::NEG_Z;
            input.axes.insert("camera.yaw".to_owned(), f64::from(forward.x.atan2(forward.z)));
            // the rest of the camera's pose, so a weapon with aim=camera can shoot through the crosshair
            let eye = rig.position();
            // the pose the camera is actually drawn with (a blend's mid-point, a fixed angle) so floating numbers land on what is on screen
            input.axes.insert("camera.pitch".to_owned(), f64::from(forward.y.clamp(-1.0, 1.0).asin()));
            input.axes.insert("camera.fov".to_owned(), f64::from(crate::viewer::fov_degrees()));
            for (name, value) in [("camera.x", eye.x), ("camera.y", eye.y), ("camera.z", eye.z)] {
                input.axes.insert(name.to_owned(), f64::from(value));
            }
            input.hands = hands_now
                .iter()
                .map(|now| {
                    let mut hand = *now;
                    if let Some(before) = last_hands.iter().find(|before| before.side == now.side) {
                        for axis in 0..3 {
                            hand.position[axis] = before.position[axis]
                                + (now.position[axis] - before.position[axis]) * blend.min(1.0);
                        }
                    }
                    hand
                })
                .collect();
            // Where things stood before the step: an effect aimed at an entity the step destroyed still has a place.
            let before: HashMap<u32, [f64; 3]> = world
                .entities()
                .iter()
                .map(|e| (e.id, e.position))
                .collect();
            match world.step(&input) {
                Ok(outcome) => {
                    *steps = steps.saturating_add(1);
                    // What the game's scripts logged, and the errors they raised, reach the page as `game.script_log` events.
                    for line in world.take_script_log() {
                        crate::world_api::push_event(
                            "game.script_log",
                            "",
                            serde_json::json!({ "text": line, "error": line.starts_with("error:") }),
                        );
                    }
                    for effect in outcome.effects {
                        let position = effect.target.and_then(|id| before.get(&id).copied());
                        // Every effect is also an event a script can listen for: `on("game.message", ({part}) => ...)` where `part` is the
                        // effect's name (`effect:message:crate_shot@a` raises game.message with part "crate_shot").
                        crate::world_api::push_event(
                            &format!("game.{}", effect_kind_name(&effect.kind)),
                            &effect.name,
                            serde_json::json!({ "target": effect.target, "value": effect.value, "position": position }),
                        );
                        if effect.kind == EffectKind::Camera
                            && let Some(rig_cam) = cameras.as_mut()
                        {
                            match rig_cam.switch(&effect.name, effect.value.unwrap_or(0.0)) {
                                Ok(()) => crate::game_camera::enter(rig_cam, world, &mut rig),
                                Err(problem) => crate::world_api::push_event("game.script_log", "", serde_json::json!({ "text": format!("error: {problem}"), "error": true })),
                            }
                        }
                        if effect.kind == EffectKind::Message {
                            crate::game_view::goto_effect(&effect.name);
                        }
                        if effect.kind == EffectKind::Particles {
                            // A burst where it happened (the rule names the effect: `explode`, `sparks`, `smoke` ...).
                            if let Some(at) = position {
                                crate::fx_bursts::request(
                                    &effect.name,
                                    at,
                                    crate::fx_bursts::GAME_SCALE,
                                );
                            }
                        }
                        if matches!(
                            effect.kind,
                            EffectKind::Anim | EffectKind::AnimSet | EffectKind::AnimFlag
                        ) {
                            // A trigger, number or flag for the animation state machine on the part this entity's kind is drawn from
                            // (every machine when the rule named no entity).
                            // An entity drawn as its own copy of a model (ADR 0066) reaches that copy's machines only.
                            let own: Option<Vec<String>> = effect
                                .target
                                .and_then(|id| models.machines_of(id))
                                .map(<[String]>::to_vec);
                            let part = effect
                                .target
                                .and_then(|id| world.entities().iter().find(|e| e.id == id))
                                .and_then(|e| world.kind(&e.kind))
                                .and_then(|kind| kind.part.clone());
                            let parts: Vec<String> =
                                own.unwrap_or_else(|| vec![part.unwrap_or_default()]);
                            for part in parts.iter().map(String::as_str) {
                                match (effect.kind.clone(), effect.value) {
                                    (EffectKind::AnimSet, Some(value)) => {
                                        #[expect(
                                            clippy::cast_possible_truncation,
                                            reason = "a parameter of an animation, well inside f32"
                                        )]
                                        crate::animation::request_float(
                                            part,
                                            &effect.name,
                                            value as f32,
                                        );
                                    }
                                    (EffectKind::AnimFlag, Some(value)) => {
                                        crate::animation::request_bool(
                                            part,
                                            &effect.name,
                                            value != 0.0,
                                        )
                                    }
                                    (EffectKind::Anim, _) => {
                                        crate::animation::request_trigger(part, &effect.name)
                                    }
                                    _ => {}
                                }
                            }
                        }
                        effects.push_back(push_effect(
                            effect.kind,
                            effect.name,
                            effect.target,
                            position,
                        ));
                        while effects.len() > EFFECT_LIMIT {
                            effects.pop_front();
                        }
                    }
                }
                Err(error) => {
                    // A failed step is rolled back and tried again next frame; say why, so a runaway script is visible instead of silent.
                    crate::world_api::push_event(
                        "game.script_log",
                        "",
                        serde_json::json!({ "text": format!("error: {error}"), "error": true }),
                    );
                    break;
                }
            }
            *accumulator -= STEP;
            count += 1;
            if world.status() != Status::Playing && !world.has_flow() {
                break;
            }
        }
        *last_hands = hands_now;
        // The game UI the scripts have set, laid out for the screen shape the page last asked about, for `game_ui_frame`.
        let frame = world.ui_frame(UI_ASPECT.with(std::cell::Cell::get));
        UI_FRAME.with(|slot| *slot.borrow_mut() = frame);
        // A game whose scripts set a camera (third person, fixed or cinematic) drives the view from here.
        // the studio's own room (floor, walls, backdrop) is not drawn around a game that has its own camera, whether a script or a
        // `camera` statement drives it: the game's world is the world
        crate::studio::set_hidden(world.script_camera().is_some() || cameras.is_some());
        if let Some((eye, target)) = world.script_camera() {
            let point = |p: [f64; 3]| bevy_math::Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32);
            let _ = crate::viewer::set_look_at(point(eye), point(target));
        }
        if let Some(camera) = cameras.as_mut() {
            crate::game_camera::drive(camera, world, &mut rig, f64::from(time.delta_secs()).min(MAX_FRAME_DELTA));
        }
        if count == MAX_STEPS {
            *accumulator = accumulator.min(STEP);
        }
        // A solid held the hero back: shove the player's body back with it.
        if count > 0
            && let Some(hero) = world.viewer_start().map(render_position)
        {
            let push = Vec3::new(hero.x - feet.x, 0.0, hero.z - feet.z);
            if push.length_squared() > 1.0e-6 {
                push_player(push, &mut rig, xr_rig.as_deref_mut());
            }
        }
    }

    let bindings = world
        .as_ref()
        .map(|world| {
            world
                .entities()
                .iter()
                .filter_map(|entity| {
                    let kind = world.kind(&entity.kind)?;
                    Some((
                        entity.id,
                        entity.kind.clone(),
                        kind.part.clone(),
                        entity.position,
                        entity.orientation,
                    ))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    // Every part a kind binds is a prototype, even while no entity of that kind is alive (a destroyed door must not reappear at the origin).
    let wanted_parts: HashSet<&str> = world
        .as_ref()
        .map(|world| {
            world
                .kinds()
                .iter()
                .filter_map(|kind| kind.part.as_deref())
                .collect()
        })
        .unwrap_or_default();
    let prototypes: HashMap<&str, Vec<(Entity, Mesh3d, VeMaterial)>> = {
        let mut result: HashMap<&str, Vec<(Entity, Mesh3d, VeMaterial)>> = HashMap::new();
        for (entity, authored, mesh, material, visibility, hidden) in &sources {
            let name = authored.name();
            if wanted_parts.contains(name) {
                let parts = result.entry(name).or_default();
                if !parts.iter().any(|(_, existing, surface)| {
                    existing.0.id() == mesh.0.id() && surface == material
                }) {
                    parts.push((entity, mesh.clone(), *material));
                }
                if hidden.is_none() {
                    commands
                        .entity(entity)
                        .insert((PrototypeVisibility(visibility.copied()), Visibility::Hidden));
                }
            } else if let Some(hidden) = hidden {
                restore_visibility(&mut commands, entity, hidden.0.as_ref());
            }
        }
        result
    };
    *hidden_prototypes = prototypes.len();
    *model_skins = models.skinned_parts();

    let mut wanted = HashSet::new();
    // in first person the camera sits inside the hero, so the hero's own body is not drawn
    let inside = world.as_ref().zip(cameras.as_ref()).and_then(|(world, cameras)| crate::game_camera::hidden_in_first_person(cameras, world));
    for (id, _, part, position, orientation) in &bindings {
        let Some(part) = part.as_deref() else {
            continue;
        };
        // A model kind is drawn as an animated copy per entity by `game_models`.
        if models.model_parts.contains(part) {
            continue;
        }
        let Some(prototypes) = prototypes.get(part) else {
            continue;
        };
        for (prototype, mesh, material) in prototypes {
            let key = (*id, *prototype);
            wanted.insert(key);
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a unit quaternion's components"
            )]
            let transform = Transform {
                translation: render_position(*position),
                rotation: bevy_math::Quat::from_xyzw(
                    orientation[0] as f32,
                    orientation[1] as f32,
                    orientation[2] as f32,
                    orientation[3] as f32,
                )
                .normalize(),
                scale: if inside == Some(*id) { Vec3::ZERO } else { Vec3::ONE },
            };
            if let Some(drawing) = drawings.get(&key).copied() {
                if let Ok(mut current) = drawing_transforms.get_mut(drawing) {
                    *current = transform;
                }
            } else {
                let drawing = commands
                    .spawn((mesh.clone(), *material, transform, GameDrawing))
                    .id();
                drawings.insert(key, drawing);
            }
        }
    }
    drawings.retain(|key, drawing| {
        if wanted.contains(key) {
            true
        } else {
            commands.entity(*drawing).despawn();
            false
        }
    });
    publish(&runtime);
}

fn restore_visibility(commands: &mut Commands, entity: Entity, old: Option<&Visibility>) {
    if let Some(visibility) = old {
        commands.entity(entity).insert(*visibility);
    } else {
        commands.entity(entity).remove::<Visibility>();
    }
    commands.entity(entity).remove::<PrototypeVisibility>();
}

fn effect_kind_name(kind: &EffectKind) -> &'static str {
    match kind {
        EffectKind::Sfx => "sfx",
        EffectKind::Particles => "particles",
        EffectKind::CameraShake => "camera_shake",
        EffectKind::Message => "message",
        EffectKind::Anim => "anim",
        EffectKind::AnimSet => "anim_set",
        EffectKind::AnimFlag => "anim_flag",
        EffectKind::Camera => "camera",
    }
}

fn effect_json(kind: EffectKind, name: String, target: Option<u32>) -> Value {
    json!({"kind": effect_kind_name(&kind), "name": name, "target": target})
}

fn publish(runtime: &GameRuntime) {
    let Some(world) = runtime.world.as_ref() else {
        STATUS.with(|status| *status.borrow_mut() = empty_status());
        return;
    };
    let status = match world.status() {
        Status::Playing => "playing",
        Status::Won => "won",
        Status::Lost => "lost",
    };
    let entities = world
        .entities()
        .iter()
        .map(|entity| {
            let part = world
                .kind(&entity.kind)
                .and_then(|kind| kind.part.as_deref());
            json!({"id":entity.id,"kind":entity.kind,"part":part,"position":entity.position})
        })
        .collect::<Vec<_>>();
    let shown = display_huds(world);
    let lines: Vec<&str> = shown.iter().map(|hud| hud.text.as_str()).collect();
    let triangles = shown
        .iter()
        .map(|hud| {
            let layout = ve_world::pixel_text::layout(&hud.text, hud_pixel_size(hud.size));
            layout.rects.len() * 2
        })
        .sum::<usize>();
    let (bursts_requested, bursts_drawn) = crate::fx_bursts::counts();
    // which game camera is active, and whether the player's look steers it (the page leaves the view alone while a game camera has it)
    let camera = runtime.cameras.as_ref().and_then(ve_game::CameraRig::active_def).map(|def| json!({"name":def.name,"mode":def.mode,"orbit":matches!(def.mode.as_str(), "first" | "third")}));
    let value = json!({"present":true,"status":status,"camera":camera,"flow":{"state":world.flow_state(),"level":world.level_name()},"steps":runtime.steps,"vars":world.vars(),
        "bursts":{"requested":bursts_requested,"particles":bursts_drawn},
        "ground":{"present":world.ground().is_some(),"samples":crate::game_ground::report().0,"error":crate::game_ground::report().1},
        "nav":{"present":world.nav().is_some(),"declared":world.nav_declaration().is_some(),"cells":crate::game_nav::report().0,"error":crate::game_nav::report().1},
        "entities":entities,"drawn":runtime.drawings.len(),"hidden_prototypes":runtime.hidden_prototypes,"model_skins":runtime.model_skins,"effects":runtime.effects,
        "hud":{"lines":lines,"triangles":triangles}});
    STATUS.with(|snapshot| *snapshot.borrow_mut() = value.to_string());
}

fn empty_status() -> String {
    json!({"present":false,"status":null,"steps":0,"vars":{},"entities":[],"drawn":0,"effects":[],
        "hud":{"lines":[],"triangles":0}})
    .to_string()
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "HUD sizes are validated metre values and pixel layout uses f32"
)]
fn hud_pixel_size(size: f64) -> f32 {
    size as f32
}
