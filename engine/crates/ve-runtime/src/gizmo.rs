//! WASM boundary for the pure pointer-ray gizmo calculations.

use std::cell::RefCell;

use bevy_math::Vec3;
use ve_interact::{GizmoConstraint, GizmoDrag, GizmoMode, GizmoSnap};

use crate::viewer;

/// Projects a selected bounds centre, three axis handles and rotation rings into canvas pixels.
pub(crate) fn project_bounds(
    min: Vec3,
    max: Vec3,
    width: f32,
    height: f32,
    pivot: Option<Vec3>,
) -> String {
    if !min.is_finite()
        || !max.is_finite()
        || !min.cmple(max).all()
        || !width.is_finite()
        || !height.is_finite()
        || width <= 0.0
        || height <= 0.0
    {
        return String::new();
    }
    let center = pivot.unwrap_or((min + max) * 0.5);
    if !center.is_finite() {
        return String::new();
    }
    let radius = ((max - min).length() * 0.5).max(0.12);
    let axis_length = radius * 1.35;
    let axes = [Vec3::X, Vec3::Y, Vec3::Z]
        .into_iter()
        .enumerate()
        .filter_map(|(axis, direction)| {
            let start = screen(center, width, height)?;
            let end = screen(center + direction * axis_length, width, height)?;
            Some(serde_json::json!({ "axis": axis, "start": start, "end": end }))
        })
        .collect::<Vec<_>>();
    let corners = (0..8)
        .map(|corner| {
            Vec3::new(
                if corner & 1 == 0 { min.x } else { max.x },
                if corner & 2 == 0 { min.y } else { max.y },
                if corner & 4 == 0 { min.z } else { max.z },
            )
        })
        .collect::<Vec<_>>();
    let bounds = (0..8)
        .flat_map(|corner| [1, 2, 4].map(move |axis| (corner, corner ^ axis)))
        .filter(|(from, to)| from < to)
        .filter_map(|(from, to)| {
            Some([
                screen(corners[from], width, height)?,
                screen(corners[to], width, height)?,
            ])
        })
        .collect::<Vec<_>>();
    let rings = [
        (Vec3::X, Vec3::Y, Vec3::Z),
        (Vec3::Y, Vec3::Z, Vec3::X),
        (Vec3::Z, Vec3::X, Vec3::Y),
    ]
    .into_iter()
    .map(|(_normal, u, v)| {
        (0_u16..=48)
            .filter_map(|step| {
                let angle = std::f32::consts::TAU * f32::from(step) / 48.0;
                screen(
                    center + (u * angle.cos() + v * angle.sin()) * radius * 0.78,
                    width,
                    height,
                )
            })
            .collect::<Vec<_>>()
    })
    .collect::<Vec<_>>();
    serde_json::json!({ "pivot": center.to_array(), "axes": axes, "rings": rings, "bounds": bounds }).to_string()
}

fn screen(point: Vec3, width: f32, height: f32) -> Option<[f32; 2]> {
    viewer::project(point, width / height).map(|projected| {
        [
            (projected.x + 1.0) * width * 0.5,
            (1.0 - projected.y) * height * 0.5,
        ]
    })
}

thread_local! {
    static DRAG: RefCell<Option<GizmoDrag>> = const { RefCell::new(None) };
}

/// Starts one world-axis translation, rotation ring or scale-axis drag.
pub(crate) fn begin(mode: &str, axis: u8, x: f32, y: f32, aspect: f32, pivot: Vec3) -> bool {
    let Some(axis) = unit_axis(axis) else {
        return false;
    };
    let Some(ray) = viewer::pointer_ray(x, y, aspect) else {
        return false;
    };
    let (mode, constraint) = match mode {
        "translate" => (
            GizmoMode::Translate,
            GizmoConstraint::Axis {
                origin: pivot,
                direction: axis,
            },
        ),
        "rotate" => (
            GizmoMode::Rotate,
            GizmoConstraint::Ring {
                center: pivot,
                normal: axis,
            },
        ),
        "scale" => (
            GizmoMode::Scale,
            GizmoConstraint::Axis {
                origin: pivot,
                direction: axis,
            },
        ),
        _ => return false,
    };
    let Some(drag) = GizmoDrag::begin(mode, constraint, &ray) else {
        return false;
    };
    DRAG.with(|active| *active.borrow_mut() = Some(drag));
    true
}

/// Starts a profile-point translation constrained to the XY plane.
pub(crate) fn begin_profile(x: f32, y: f32, aspect: f32, point: Vec3) -> bool {
    let Some(ray) = viewer::pointer_ray(x, y, aspect) else {
        return false;
    };
    let constraint = GizmoConstraint::Plane {
        origin: point,
        normal: Vec3::Z,
    };
    let Some(drag) = GizmoDrag::begin(GizmoMode::Translate, constraint, &ray) else {
        return false;
    };
    DRAG.with(|active| *active.borrow_mut() = Some(drag));
    true
}

/// Starts a control-point drag in the plane facing the desktop camera at the pointer.
pub(crate) fn begin_camera_plane(x: f32, y: f32, aspect: f32, point: Vec3) -> bool {
    let Some(ray) = viewer::pointer_ray(x, y, aspect) else {
        return false;
    };
    let constraint = GizmoConstraint::Plane {
        origin: point,
        normal: ray.direction,
    };
    let Some(drag) = GizmoDrag::begin(GizmoMode::Translate, constraint, &ray) else {
        return false;
    };
    DRAG.with(|active| *active.borrow_mut() = Some(drag));
    true
}

/// Samples the current pointer and encodes the delta as a compact JSON tagged object.
pub(crate) fn update(x: f32, y: f32, aspect: f32, grid: f32, angle: f32, scale: f32) -> String {
    let Some(ray) = viewer::pointer_ray(x, y, aspect) else {
        return String::new();
    };
    DRAG.with(|active| {
        let mut active = active.borrow_mut();
        let Some(drag) = active.as_mut() else {
            return String::new();
        };
        let delta = drag.update(
            &ray,
            GizmoSnap {
                grid: positive(grid),
                angle: positive(angle),
                scale: positive(scale),
            },
        );
        let Some(delta) = delta else {
            return String::new();
        };
        match delta {
            ve_interact::GizmoDelta::Translation(value) => {
                serde_json::json!({ "kind": "translate", "value": value.to_array() })
            }
            ve_interact::GizmoDelta::Rotation { axis, radians } => {
                serde_json::json!({ "kind": "rotate", "axis": axis.to_array(), "radians": radians })
            }
            ve_interact::GizmoDelta::Scale { axis, factor } => {
                serde_json::json!({ "kind": "scale", "axis": axis.to_array(), "factor": factor })
            }
        }
        .to_string()
    })
}

/// Drops an unfinished drag; no document or scene entity is touched.
pub(crate) fn cancel() {
    DRAG.with(|active| *active.borrow_mut() = None);
}

fn unit_axis(axis: u8) -> Option<Vec3> {
    match axis {
        0 => Some(Vec3::X),
        1 => Some(Vec3::Y),
        2 => Some(Vec3::Z),
        _ => None,
    }
}

fn positive(value: f32) -> Option<f32> {
    (value.is_finite() && value > 0.0).then_some(value)
}
