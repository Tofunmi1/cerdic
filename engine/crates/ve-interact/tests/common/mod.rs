//! A tiny world of spheres and the helpers the interaction tests share.
#![expect(
    dead_code,
    reason = "each test binary uses a different subset of these helpers"
)]

use std::cell::RefCell;
use std::collections::BTreeMap;

use glam::{Quat, Vec3};
use ve_interact::{
    Hit, Interaction, Output, Placement3, PointerFrame, PointerId, Pose, Ray, Shape, TargetId,
    World,
};

pub(crate) const MOUSE: PointerId = PointerId(0);
pub(crate) const LEFT: PointerId = PointerId(1);
pub(crate) const RIGHT: PointerId = PointerId(2);
pub(crate) const BALL: TargetId = TargetId(1);
pub(crate) const OTHER: TargetId = TargetId(2);

pub(crate) struct Object {
    pub(crate) radius: f32,
    pub(crate) placement: Placement3,
    pub(crate) grabbable: bool,
}

#[derive(Default)]
pub(crate) struct Scene {
    pub(crate) objects: BTreeMap<TargetId, Object>,
    pub(crate) snaps: RefCell<Vec<(TargetId, Placement3)>>,
    pub(crate) snap_to: Option<Placement3>,
}

impl Scene {
    /// A scene with one half-metre ball at the origin.
    pub(crate) fn with_ball() -> Self {
        let mut scene = Self::default();
        scene.add(BALL, Vec3::ZERO, 0.5);
        scene
    }

    pub(crate) fn add(&mut self, id: TargetId, at: Vec3, radius: f32) {
        self.objects.insert(
            id,
            Object {
                radius,
                placement: Placement3 {
                    translation: at,
                    ..Placement3::IDENTITY
                },
                grabbable: true,
            },
        );
    }

    /// Applies the placements the machine asked for, as a host would.
    pub(crate) fn apply(&mut self, output: &Output) {
        for (target, placement) in &output.placements {
            if let Some(object) = self.objects.get_mut(target) {
                object.placement = *placement;
            }
        }
    }

    pub(crate) fn placement_of(&self, target: TargetId) -> Placement3 {
        self.objects[&target].placement
    }
}

impl World for Scene {
    fn pick(&self, ray: &Ray, max_distance: f32) -> Option<Hit> {
        self.objects
            .iter()
            .filter_map(|(id, object)| {
                let shape = Shape::Sphere {
                    center: object.placement.translation,
                    radius: object.radius * object.placement.scale,
                };
                let distance = shape.raycast(ray)?;
                (distance <= max_distance).then(|| Hit {
                    target: *id,
                    distance,
                    point: ray.at(distance),
                })
            })
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
    }

    fn placement(&self, target: TargetId) -> Option<Placement3> {
        self.objects.get(&target).map(|object| object.placement)
    }

    fn grabbable(&self, target: TargetId) -> bool {
        self.objects
            .get(&target)
            .is_some_and(|object| object.grabbable)
    }

    fn snap(&self, target: TargetId, released: Placement3) -> Option<Placement3> {
        self.snaps.borrow_mut().push((target, released));
        self.snap_to
    }
}

/// A ray starting at `(x, y, 5)` and looking down -z.
pub(crate) fn ray_at(x: f32, y: f32) -> Option<Ray> {
    Ray::new(Vec3::new(x, y, 5.0), Vec3::NEG_Z)
}

/// A mouse-like pointer: ray only, and pressing the button grabs.
pub(crate) fn mouse(x: f32, y: f32, down: bool) -> PointerFrame {
    PointerFrame {
        ray: ray_at(x, y),
        select: down,
        select_grabs: true,
        ..PointerFrame::idle(MOUSE)
    }
}

/// A controller held at `grip`, pointing down -z, with the given buttons.
pub(crate) fn controller(
    id: PointerId,
    grip: Vec3,
    rotation: Quat,
    squeeze: bool,
    trigger: bool,
) -> PointerFrame {
    PointerFrame {
        ray: Ray::new(grip, Vec3::NEG_Z),
        grip: Some(Pose {
            position: grip,
            rotation,
        }),
        select: trigger,
        squeeze,
        ..PointerFrame::idle(id)
    }
}

/// One frame: update the machine and apply its placements to the scene.
pub(crate) fn step(
    machine: &mut Interaction,
    scene: &mut Scene,
    frames: &[PointerFrame],
) -> Output {
    let output = machine.update(frames, scene);
    scene.apply(&output);
    output
}
