//! Stereo for Bevy's pipeline. The runtime gives the headset (or mock headset) camera a [`StereoViews`] component with two eyes, each a
//! pose, a projection and a viewport of the camera's target. Bevy draws one view per camera, so while that component is present the main
//! camera is switched off and two eye cameras take its place, each with the eye's own viewport and projection matrix, aimed at the same target.

use bevy_app::{App, Plugin, Update};
use bevy_camera::{
    Camera, Camera3d, CameraProjection, Projection, RenderTarget, SubCameraView, Viewport,
};
use bevy_ecs::prelude::*;
use bevy_math::{Mat4, UVec2, Vec3A, Vec4};
use bevy_transform::components::Transform;
use ve_render::{StereoEye, StereoViews};

/// Registers the stereo systems.
#[derive(Debug, Default, Clone, Copy)]
pub struct VeBevyStereoPlugin;

impl Plugin for VeBevyStereoPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EyeCameras>()
            .add_systems(Update, map_stereo);
    }
}

/// A camera that draws one eye of a stereo view.
#[derive(Component)]
pub(crate) struct EyeCamera;

/// The main camera was switched off because its eyes are drawn by eye cameras.
#[derive(Component)]
struct SuppressedByStereo;

/// The two eye cameras while a stereo view exists.
#[derive(Resource, Default)]
struct EyeCameras([Option<Entity>; 2]);

/// A projection that is exactly the matrix the headset reported (reverse-z, infinite far plane, possibly asymmetric).
#[derive(Clone, Debug)]
pub struct EyeProjection {
    clip_from_view: Mat4,
}

impl EyeProjection {
    /// The projection with this `clip_from_view` matrix.
    #[must_use]
    pub fn new(clip_from_view: Mat4) -> Self {
        Self { clip_from_view }
    }

    /// NDC depth of the plane `distance` metres in front of the eye.
    fn ndc_depth(&self, distance: f32) -> f32 {
        let clip = self.clip_from_view * Vec4::new(0.0, 0.0, -distance, 1.0);
        clip.z / clip.w
    }
}

impl CameraProjection for EyeProjection {
    fn get_clip_from_view(&self) -> Mat4 {
        self.clip_from_view
    }

    fn get_clip_from_view_for_sub(&self, _sub_view: &SubCameraView) -> Mat4 {
        self.clip_from_view
    }

    fn update(&mut self, _width: f32, _height: f32) {}

    fn far(&self) -> f32 {
        // The projection has no far plane; this is only what distance-based settings (shadow cascades) clamp to.
        1000.0
    }

    fn get_frustum_corners(&self, z_near: f32, z_far: f32) -> [Vec3A; 8] {
        let view_from_clip = self.clip_from_view.inverse();
        let corner = |x: f32, y: f32, distance: f32| {
            let view = view_from_clip * Vec4::new(x, y, self.ndc_depth(distance), 1.0);
            Vec3A::from_vec4(view / view.w)
        };
        let ring = |distance: f32| {
            [
                corner(1.0, -1.0, distance),
                corner(1.0, 1.0, distance),
                corner(-1.0, 1.0, distance),
                corner(-1.0, -1.0, distance),
            ]
        };
        let (near, far) = (ring(z_near), ring(z_far));
        [
            near[0], near[1], near[2], near[3], far[0], far[1], far[2], far[3],
        ]
    }
}

fn viewport_of(eye: &StereoEye) -> Viewport {
    let [x, y, width, height] = eye.viewport;
    Viewport {
        physical_position: UVec2::new(x, y),
        physical_size: UVec2::new(width.max(1), height.max(1)),
        ..Viewport::default()
    }
}

/// Keeps the eye cameras in step with the stereo camera: spawned while it has eyes, updated every frame, despawned when it loses them.
fn map_stereo(
    mut commands: Commands,
    mut eyes: ResMut<EyeCameras>,
    mut mains: Query<
        (
            Entity,
            Option<&StereoViews>,
            &RenderTarget,
            &mut Camera,
            Has<SuppressedByStereo>,
        ),
        (With<Camera3d>, Without<EyeCamera>),
    >,
    mut eye_cameras: Query<
        (
            &mut Camera,
            &mut Transform,
            &mut Projection,
            &mut RenderTarget,
        ),
        With<EyeCamera>,
    >,
) {
    let stereo = mains
        .iter()
        .find_map(|(_, views, target, _, _)| views.map(|views| (views.eyes, target.clone())));
    let Some((views, target)) = stereo else {
        for slot in &mut eyes.0 {
            if let Some(entity) = slot.take() {
                commands.entity(entity).despawn();
            }
        }
        for (entity, _, _, mut camera, suppressed) in &mut mains {
            if suppressed {
                camera.is_active = true;
                commands.entity(entity).remove::<SuppressedByStereo>();
            }
        }
        return;
    };
    for (entity, views, _, mut camera, suppressed) in &mut mains {
        if views.is_some() && !suppressed {
            camera.is_active = false;
            commands.entity(entity).insert(SuppressedByStereo);
        }
    }
    for (index, eye) in views.iter().enumerate() {
        let transform = Transform::from_matrix(eye.world_from_view);
        let projection = Projection::custom(EyeProjection::new(eye.clip_from_view));
        let live = eyes.0[index].filter(|entity| eye_cameras.contains(*entity));
        if let Some(entity) = live
            && let Ok((mut camera, mut current, mut current_projection, mut current_target)) =
                eye_cameras.get_mut(entity)
        {
            camera.viewport = Some(viewport_of(eye));
            *current = transform;
            *current_projection = projection;
            *current_target = target.clone();
            continue;
        }
        let entity = commands
            .spawn((
                EyeCamera,
                Camera3d::default(),
                Camera {
                    viewport: Some(viewport_of(eye)),
                    order: 100 + isize::try_from(index).unwrap_or(0),
                    ..Camera::default()
                },
                target.clone(),
                transform,
                projection,
            ))
            .id();
        eyes.0[index] = Some(entity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reverse-z, infinite far plane, 90 degrees vertical, the given aspect ratio: what `ve-xr-math` produces for a symmetric eye.
    fn reverse_z(aspect: f32, near: f32) -> Mat4 {
        Mat4::from_cols(
            Vec4::new(1.0 / aspect, 0.0, 0.0, 0.0),
            Vec4::new(0.0, 1.0, 0.0, 0.0),
            Vec4::new(0.0, 0.0, 0.0, -1.0),
            Vec4::new(0.0, 0.0, near, 0.0),
        )
    }

    #[test]
    fn frustum_corners_lie_on_the_planes_at_the_asked_distances() {
        let projection = EyeProjection::new(reverse_z(2.0, 0.1));
        let corners = projection.get_frustum_corners(1.0, 4.0);
        let expect = |corner: Vec3A, x: f32, y: f32, z: f32| {
            assert!(
                corner.abs_diff_eq(Vec3A::new(x, y, z), 1e-4),
                "{corner} against ({x}, {y}, {z})"
            );
        };
        // 90 degrees vertically: half height equals the distance; aspect 2: half width is twice that.
        expect(corners[0], 2.0, -1.0, -1.0);
        expect(corners[1], 2.0, 1.0, -1.0);
        expect(corners[2], -2.0, 1.0, -1.0);
        expect(corners[3], -2.0, -1.0, -1.0);
        expect(corners[4], 8.0, -4.0, -4.0);
        expect(corners[6], -8.0, 4.0, -4.0);
    }

    #[test]
    fn the_projection_is_the_reported_matrix_untouched() {
        let matrix = reverse_z(1.3, 0.05);
        let projection = EyeProjection::new(matrix);
        assert_eq!(projection.get_clip_from_view(), matrix);
        assert_eq!(
            projection.get_clip_from_view_for_sub(&SubCameraView::default()),
            matrix
        );
    }
}
