use std::collections::BTreeSet;

use bevy_app::{App, Update};
use bevy_camera::{Camera, Camera3d};
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::IntoScheduleConfigs;
use bevy_ecs::query::{With, Without};
use bevy_ecs::resource::Resource;
use bevy_ecs::system::{Query, Res, ResMut};
use bevy_input::ButtonInput;
use bevy_input::mouse::{MouseButton, MouseScrollUnit, MouseWheel};
use bevy_math::{Quat, Vec2, Vec3};
use bevy_time::{Real, Time};
use bevy_transform::components::{GlobalTransform, Transform};
use bevy_window::{CursorLeft, CursorMoved};
use ve_interact::{
    Event, Hit, Interaction, Placement3, PointerFrame, PointerId, Ray, Shape, TargetId, World,
};
use ve_kernel::Bounds;
use ve_render::StereoViews;
use ve_xr_web::frame::Handedness;
use ve_xr_web::{XrInput, XrRig, apply_xr_frame, controller_pointer};

use crate::viewer::{ViewerRig, update_viewer};

mod hover;
mod report;
mod spawn;

pub(crate) use hover::highlight_hover;
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
    /// Authored geometry is selectable; editing its pose requires a document command.
    pub(crate) fn selection_only(bounds: &Bounds) -> Self {
        Self {
            grabbable: false,
            ..Self::from_bounds(bounds)
        }
    }
    /// Lets or stops hands picking the object up (authored parts are grabbable only in play mode, and only when
    /// a behaviour asks to be grabbed).
    pub(crate) fn set_grabbable(&mut self, on: bool) {
        self.grabbable = on;
    }

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
#[derive(Component, Clone, Debug)]
pub(crate) struct Label(pub(crate) String);

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
    /// Objects clicked since the selection last looked.
    clicked: Vec<Entity>,
    /// Objects picked up (true) or let go (false) since behaviours last looked.
    grabs: Vec<(Entity, bool)>,
    log: Vec<String>,
}

impl InteractionState {
    fn new() -> Self {
        Self {
            machine: Interaction::new(PICK_DISTANCE),
            hovered: BTreeSet::new(),
            clicked: Vec::new(),
            grabs: Vec::new(),
            log: Vec::new(),
        }
    }

    /// The objects clicked since the last call.
    pub(crate) fn take_clicked(&mut self) -> Vec<Entity> {
        std::mem::take(&mut self.clicked)
    }

    /// The objects picked up (true) or let go (false) since the last call.
    pub(crate) fn take_grabs(&mut self) -> Vec<(Entity, bool)> {
        std::mem::take(&mut self.grabs)
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
    objects: &'a Query<
        'w,
        's,
        (
            Entity,
            &'static Interactable,
            &'static Transform,
            Option<&'static ve_render::VeInstanceTransform>,
        ),
    >,
    /// Retained instance sets, picked copy by copy through [`ve_interact::BoxSet`] (and left out of `objects`).
    bulk: &'a Query<'w, 's, (Entity, &'static crate::projection::bulk::BulkInstances)>,
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
        let sets = self.bulk.iter().filter_map(|(entity, set)| {
            let (distance, _) = set.0.pick(ray)?;
            (distance <= max_distance).then(|| Hit {
                target: target_of(entity),
                distance,
                point: ray.at(distance),
            })
        });
        self.objects
            .iter()
            .filter(|(entity, ..)| !self.bulk.contains(*entity))
            .filter_map(|(entity, interactable, transform, placement)| {
                // An object whose transform a box cannot represent (collapsed, mirrored, sheared)
                // is not pickable until it is placed validly again.
                let distance = if let Some(placement) = placement {
                    ve_interact::ray_affine_box(
                        ray,
                        interactable.min,
                        interactable.max,
                        placement.0,
                    )
                } else {
                    Shape::from_local_bounds(
                        interactable.min,
                        interactable.max,
                        transform.to_matrix(),
                    )?
                    .raycast(ray)
                }
                .filter(|d| *d <= max_distance)?;
                Some(Hit {
                    target: target_of(entity),
                    distance,
                    point: ray.at(distance),
                })
            })
            .chain(sets)
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
    }

    fn placement(&self, target: TargetId) -> Option<Placement3> {
        let (_, _, transform, _) = self.objects.get(entity_of(target)?).ok()?;
        Some(placement_of(transform))
    }

    fn grabbable(&self, target: TargetId) -> bool {
        entity_of(target)
            .and_then(|entity| self.objects.get(entity).ok())
            .is_some_and(|(_, interactable, _, _)| interactable.grabbable)
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
        .init_resource::<crate::behavior::Behaviors>()
        .insert_resource(InteractionState::new())
        .add_systems(
            Update,
            (
                update_viewer,
                track_mouse,
                run_interaction,
                crate::selection::select_on_click,
                highlight_hover,
            )
                .chain()
                .after(apply_xr_frame),
        );
}

/// Remembers the cursor position and accumulates wheel movement between frames.
fn track_mouse(
    mut cursor: ResMut<CursorState>,
    mut moved: MessageReader<CursorMoved>,
    mut left: MessageReader<CursorLeft>,
    mut wheel: MessageReader<MouseWheel>,
) {
    // In arrival order is not kept across the two readers; a leave after a move in one frame wins,
    // which is the right outcome (the pointer ended up outside).
    for event in moved.read() {
        cursor.position = Some(event.position);
    }
    if left.read().count() > 0 {
        cursor.position = None;
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
