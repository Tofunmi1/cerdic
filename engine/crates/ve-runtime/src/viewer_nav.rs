//! Inspecting a model with the desktop viewer: zoom toward a focus point, orbit around it, and frame
//! the whole model. The page feeds in wheel and drag movement; the viewer applies it once a frame.
//! The focus is whatever the page last asked to look at (usually the evaluated model's bounds).

use std::cell::{Cell, RefCell};

use bevy_math::{Vec2, Vec3};

/// What the viewer looks at and how big it is.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Focus {
    pub(crate) center: Vec3,
    pub(crate) radius: f32,
}

impl Default for Focus {
    fn default() -> Self {
        Self {
            center: Vec3::new(0.0, 0.5, 0.0),
            radius: 2.0,
        }
    }
}

#[derive(Default)]
struct Pending {
    zoom: f32,
    orbit: Vec2,
    frame: Option<Focus>,
}

thread_local! {
    static PENDING: RefCell<Pending> = RefCell::new(Pending::default());
    static FOCUS: Cell<Focus> = Cell::new(Focus::default());
    static ROOM_SCALE: Cell<f32> = const { Cell::new(1.0) };
}

/// Half the studio room's width at its normal size, in metres.
const ROOM_HALF_WIDTH: f32 = 7.0;

/// How much bigger than normal the studio room should be so a camera framing the focus stays inside
/// its walls (1 for models that already fit).
pub(crate) fn room_scale() -> f32 {
    ROOM_SCALE.with(Cell::get)
}

fn with_pending<T>(f: impl FnOnce(&mut Pending) -> T) -> T {
    PENDING.with(|p| f(&mut p.borrow_mut()))
}

/// Wheel movement to zoom by on the next frame (positive zooms out). One notch is about 100.
pub(crate) fn zoom(amount: f32) {
    if amount.is_finite() {
        with_pending(|p| p.zoom += amount);
    }
}

/// Drag movement in pixels to orbit by on the next frame.
pub(crate) fn orbit(dx: f32, dy: f32) {
    if dx.is_finite() && dy.is_finite() {
        with_pending(|p| p.orbit += Vec2::new(dx, dy));
    }
}

/// Moves the camera so a sphere of `radius` around `center` fills the view, from a three-quarter
/// angle, and makes it the focus.
pub(crate) fn frame(center: Vec3, radius: f32) -> bool {
    let ok = center.is_finite() && radius.is_finite() && radius > 0.0;
    if ok {
        with_pending(|p| p.frame = Some(Focus { center, radius }));
    }
    ok
}

/// Yaw and pitch that look from `from` at `to`.
pub(crate) fn aim(from: Vec3, to: Vec3) -> (f32, f32) {
    let direction = to - from;
    let length = direction.length().max(1.0e-9);
    (
        (-direction.x).atan2(-direction.z),
        (direction.y / length).asin(),
    )
}

/// Applies everything the page asked for since the last frame to the camera's position and angles.
pub(crate) fn apply(position: &mut Vec3, yaw: &mut f32, pitch: &mut f32, vertical_fov: f32) {
    let pending = with_pending(std::mem::take);
    let mut focus = FOCUS.with(Cell::get);
    if let Some(next) = pending.frame {
        focus = next;
        let distance = next.radius / (vertical_fov * 0.5).sin().max(0.05) * 1.1;
        // Keep the camera, and the model's own footprint, inside the room's walls.
        let reach = distance.max(next.center.x.abs().max(next.center.z.abs()) + next.radius);
        ROOM_SCALE.with(|scale| scale.set((reach * 1.4 / ROOM_HALF_WIDTH).max(1.0)));
        let (azimuth, elevation) = (35.0_f32.to_radians(), 20.0_f32.to_radians());
        *position = next.center
            + distance
                * Vec3::new(
                    azimuth.sin() * elevation.cos(),
                    elevation.sin(),
                    azimuth.cos() * elevation.cos(),
                );
        (*yaw, *pitch) = aim(*position, next.center);
    }
    if pending.orbit != Vec2::ZERO {
        let offset = *position - focus.center;
        let distance = offset.length();
        if distance > 1.0e-4 {
            let azimuth = offset.x.atan2(offset.z) - pending.orbit.x * 0.006;
            let elevation =
                ((offset.y / distance).asin() + pending.orbit.y * 0.006).clamp(-1.45, 1.45);
            *position = focus.center
                + distance
                    * Vec3::new(
                        azimuth.sin() * elevation.cos(),
                        elevation.sin(),
                        azimuth.cos() * elevation.cos(),
                    );
            (*yaw, *pitch) = aim(*position, focus.center);
        }
    }
    if pending.zoom != 0.0 {
        let distance = (focus.center - *position).length();
        if distance > 1.0e-4 {
            let wanted = (distance * (pending.zoom * 0.0015).exp())
                .clamp(focus.radius * 0.02, focus.radius * 40.0);
            let forward = bevy_math::Quat::from_euler(bevy_math::EulerRot::YXZ, *yaw, *pitch, 0.0)
                * Vec3::NEG_Z;
            *position += forward * (distance - wanted);
        }
    }
    FOCUS.with(|f| f.set(focus));
}
