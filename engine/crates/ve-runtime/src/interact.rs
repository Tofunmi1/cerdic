//! Interaction in the running app: objects marked [`Interactable`] can be hovered, clicked, grabbed
//! and (with two hands) scaled and turned, by mouse or by controllers, through one state machine
//! (`ve-interact`). This module is the glue: it gathers pointers, lets the machine run, applies the
//! placements it returns, tints what is hovered and draws each controller's laser.

use std::collections::BTreeSet;

use bevy_app::{App, Update};
use bevy_camera::{Camera, Camera3d};
use bevy_color::{Color, LinearRgba};
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::IntoScheduleConfigs;
use bevy_ecs::query::{With, Without};
use bevy_ecs::resource::Resource;
use bevy_ecs::system::{Commands, Query, Res, ResMut};
use bevy_input::ButtonInput;
use bevy_input::mouse::{MouseButton, MouseScrollUnit, MouseWheel};
use bevy_math::{Quat, Vec2, Vec3};
use bevy_time::{Real, Time};
use bevy_transform::components::{GlobalTransform, Transform};
use bevy_window::CursorMoved;
use ve_interact::{
    Hit, Interaction, Placement3, PointerFrame, PointerId, Ray, Shape, TargetId, World,
};
use ve_kernel::Bounds;
use ve_render::{StereoViews, VeMaterial};
use ve_xr_web::frame::Handedness;
use ve_xr_web::{XrInput, XrRig, apply_xr_frame, controller_pointer};

use crate::viewer::{ViewerRig, update_viewer};

mod report;
mod spawn;

use report::describe;
pub(crate) use report::report;
pub(crate) use spawn::{spawn_lasers, tag};

/// How far pointers can pick, in metres.
const PICK_DISTANCE: f32 = 50.0;

/// How long a controller's laser is when it points at nothing, and its longest length.
const LASER_REST: f32 = 3.0;

/// Metres of push or pull per mouse-wheel line, and per pixel of wheel delta.
const WHEEL_LINE: f32 = 0.25;
const WHEEL_PIXEL: f32 = 0.002;

/// How much of an object's colour is added as glow while it is hovered.
const HOVER_GLOW: f32 = 0.35;

/// Events kept for the status report.
const LOG_LINES: usize = 24;

/// An object pointers can pick and hold. Its local-space bounds define where rays hit it. It must
/// not have a parent: its [`Transform`] is read and written as a world placement.
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct Interactable {
    min: Vec3,
    max: Vec3,
    grabbable: bool,
}

impl Interactable {
    /// An interactable object whose shape is the bounding box of `bounds`.
    pub(crate) fn from_bounds(bounds: &Bounds) -> Self {
        #[expect(clippy::cast_possible_truncation, reason = "metres fit f32")]
        let vec = |v: [f64; 3]| Vec3::new(v[0] as f32, v[1] as f32, v[2] as f32);
        Self {
            min: vec(bounds.min),
            max: vec(bounds.max),
            grabbable: true,
        }
    }
}

/// A short name for an object, for the status report and the event log.
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct Label(pub(crate) &'static str);

/// Marks an object whose emissive colour is currently raised for hover, remembering the original.
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct HoverTint(Color);

/// A controller's laser pointer.
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct Laser(pub(crate) Handedness);

/// The mouse as the platform layer reports it.
#[derive(Resource, Default)]
pub(crate) struct CursorState {
    position: Option<Vec2>,
    reach: f32,
}

/// The socket-snap hook: where a released object should settle instead of where it was let go. The
/// snapping rules belong to later work; by default nothing snaps.
#[derive(Resource, Default)]
pub(crate) struct SnapHook(
    pub(crate) Option<Box<dyn Fn(Entity, Placement3) -> Option<Placement3> + Send + Sync>>,
);

/// The state machine and what it did lately.
#[derive(Resource)]
pub(crate) struct InteractionState {
    machine: Interaction,
    hovered: BTreeSet<Entity>,
    log: Vec<String>,
}

impl InteractionState {
    fn new() -> Self {
        Self {
            machine: Interaction::new(PICK_DISTANCE),
            hovered: BTreeSet::new(),
            log: Vec::new(),
        }
    }

    fn note(&mut self, line: String) {
        if self.log.len() == LOG_LINES {
            self.log.remove(0);
        }
        self.log.push(line);
    }
}

fn target_of(entity: Entity) -> TargetId {
    TargetId(entity.to_bits())
}

fn entity_of(target: TargetId) -> Option<Entity> {
    Entity::try_from_bits(target.0)
}

/// The interaction machine's view of the scene: every [`Interactable`] and its transform.
struct SceneWorld<'a, 'w, 's> {
    objects: &'a Query<'w, 's, (Entity, &'static Interactable, &'static Transform)>,
    hook: &'a SnapHook,
}

fn placement_of(transform: &Transform) -> Placement3 {
    Placement3 {
        translation: transform.translation,
        rotation: transform.rotation,
        scale: transform.scale.x,
    }
}

impl World for SceneWorld<'_, '_, '_> {
    fn pick(&self, ray: &Ray, max_distance: f32) -> Option<Hit> {
        self.objects
            .iter()
            .filter_map(|(entity, interactable, transform)| {
                // An object whose transform a box cannot represent (collapsed, mirrored, sheared)
                // is not pickable until it is placed validly again.
                let shape = Shape::from_local_bounds(
                    interactable.min,
                    interactable.max,
                    transform.to_matrix(),
                )?;
                let distance = shape.raycast(ray).filter(|d| *d <= max_distance)?;
                Some(Hit {
                    target: target_of(entity),
                    distance,
                    point: ray.at(distance),
                })
            })
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
    }

    fn placement(&self, target: TargetId) -> Option<Placement3> {
        let (_, _, transform) = self.objects.get(entity_of(target)?).ok()?;
        Some(placement_of(transform))
    }

    fn grabbable(&self, target: TargetId) -> bool {
        entity_of(target)
            .and_then(|entity| self.objects.get(entity).ok())
            .is_some_and(|(_, interactable, _)| interactable.grabbable)
    }

    fn snap(&self, target: TargetId, released: Placement3) -> Option<Placement3> {
        let hook = self.hook.0.as_ref()?;
        hook(entity_of(target)?, released)
    }
}

/// Registers the interaction resources and systems.
pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<CursorState>()
        .init_resource::<SnapHook>()
        .insert_resource(InteractionState::new())
        .add_systems(
            Update,
            (update_viewer, track_mouse, run_interaction, highlight_hover)
                .chain()
                .after(apply_xr_frame),
        );
}

/// Remembers the cursor position and accumulates wheel movement between frames.
fn track_mouse(
    mut cursor: ResMut<CursorState>,
    mut moved: MessageReader<CursorMoved>,
    mut wheel: MessageReader<MouseWheel>,
) {
    for event in moved.read() {
        cursor.position = Some(event.position);
    }
    for event in wheel.read() {
        let per_unit = match event.unit {
            MouseScrollUnit::Line => WHEEL_LINE,
            MouseScrollUnit::Pixel => WHEEL_PIXEL,
        };
        // Scrolling up (negative y) pushes away from the viewer.
        cursor.reach -= event.y * per_unit;
    }
}

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
fn run_interaction(
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
    mut objects: Query<(Entity, &'static Interactable, &'static mut Transform)>,
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
    }
    state.hovered = (0..=3_u32)
        .filter_map(|id| state.machine.hover_of(PointerId(id)))
        .filter_map(entity_of)
        .collect();

    for (target, placement) in &output.placements {
        if let Some(entity) = entity_of(*target)
            && let Ok((_, _, mut transform)) = objects.get_mut(entity)
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

/// Raises the emissive colour of hovered objects and puts it back afterwards.
pub(crate) fn highlight_hover(
    mut commands: Commands,
    state: Res<InteractionState>,
    mut objects: Query<(Entity, &mut VeMaterial, Option<&HoverTint>), With<Interactable>>,
) {
    for (entity, mut material, tint) in &mut objects {
        let hovered = state.hovered.contains(&entity);
        match (hovered, tint) {
            (true, None) => {
                commands.entity(entity).insert(HoverTint(material.emissive));
                let base = LinearRgba::from(material.base_color);
                material.emissive = Color::linear_rgb(
                    base.red * HOVER_GLOW,
                    base.green * HOVER_GLOW,
                    base.blue * HOVER_GLOW,
                );
            }
            (false, Some(HoverTint(original))) => {
                material.emissive = *original;
                commands.entity(entity).remove::<HoverTint>();
            }
            _ => {}
        }
    }
}
