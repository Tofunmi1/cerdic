use bevy_asset::uuid::Uuid;
use bevy_math::Vec3;

use super::*;
use crate::material::VeMaterial;

fn mesh_id(n: u128) -> AssetId<Mesh> {
    AssetId::Uuid {
        uuid: Uuid::from_u128(n),
    }
}

fn instance(mesh: u128, x: f32) -> ExtractedMesh {
    ExtractedMesh {
        mesh: mesh_id(mesh),
        world_from_local: Mat4::from_translation(Vec3::new(x, 0.0, 0.0)),
        material: VeMaterial::default().pack_for_instance(0),
        transparent: false,
        sort_key: u64::try_from(mesh).unwrap_or(u64::MAX),
        center: Vec3::new(x, 0.0, 0.0),
    }
}

#[test]
fn no_instances_gives_no_batches() {
    let (data, batches) = group_into_batches(&[]);
    assert!(data.is_empty() && batches.is_empty());
}

#[test]
fn one_batch_per_distinct_mesh_covering_all_instances() {
    let input = [
        instance(2, 0.0),
        instance(1, 1.0),
        instance(2, 2.0),
        instance(1, 3.0),
        instance(3, 4.0),
    ];
    let (data, batches) = group_into_batches(&input);
    assert_eq!(data.len(), 5);
    assert_eq!(batches.len(), 3);
    let covered: u32 = batches
        .iter()
        .map(|b| b.instances.end - b.instances.start)
        .sum();
    assert_eq!(covered, 5);
    // Batches are contiguous, in order, and start at zero.
    assert_eq!(batches[0].instances.start, 0);
    for pair in batches.windows(2) {
        assert_eq!(pair[0].instances.end, pair[1].instances.start);
    }
}

#[test]
fn instance_data_follows_batch_order_and_keeps_extraction_order_within_a_mesh() {
    let mut input = [instance(2, 10.0), instance(1, 20.0), instance(2, 30.0)];
    input[0].sort_key = 99;
    input[2].sort_key = 1;
    let (data, batches) = group_into_batches(&input);
    // Mesh 1 sorts first with one instance; mesh 2 follows with its two in extraction order.
    assert_eq!(batches[0].mesh, mesh_id(1));
    assert_eq!(batches[0].instances, 0..1);
    assert_eq!(batches[1].instances, 1..3);
    let translation_x = |i: usize| data[i].world_from_local[3][0];
    assert!((translation_x(0) - 20.0).abs() < f32::EPSILON);
    assert!((translation_x(1) - 10.0).abs() < f32::EPSILON);
    assert!((translation_x(2) - 30.0).abs() < f32::EPSILON);
}

#[test]
fn view_uniform_inverts_the_pose_and_records_the_eye_position() {
    // Any invertible matrix works as a stand-in projection for checking the pose handling.
    let projection = Mat4::from_scale(Vec3::new(2.0, 3.0, 4.0));
    let pose = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
    let uniform = ViewUniform::new(pose, projection);
    // A point at the eye's own position projects like the view-space origin: onto the axis.
    let at_eye = uniform.clip_from_world * Vec3::new(1.0, 2.0, 3.0).extend(1.0);
    let at_origin = projection * Vec3::ZERO.extend(1.0);
    assert!((at_eye - at_origin).abs().max_element() < 1e-5);
    assert!((uniform.camera_position.truncate() - Vec3::new(1.0, 2.0, 3.0)).length() < 1e-6);
    assert!((uniform.camera_position.w - 1.0).abs() < f32::EPSILON);
}

#[test]
fn different_materials_on_one_mesh_share_a_batch() {
    let mut shiny = instance(1, 0.0);
    shiny.material = VeMaterial {
        metallic: 1.0,
        roughness: 0.1,
        ..VeMaterial::default()
    }
    .pack_for_instance(0);
    let (data, batches) = group_into_batches(&[shiny, instance(1, 1.0)]);
    assert_eq!(batches.len(), 1);
    assert!((data[0].params[0] - 1.0).abs() < f32::EPSILON);
    assert!(data[1].params[0].abs() < f32::EPSILON);
}

#[path = "prepare/tests/foliage_tests.rs"]
mod foliage_tests;
#[path = "prepare/tests/opacity_tests.rs"]
mod opacity_tests;
