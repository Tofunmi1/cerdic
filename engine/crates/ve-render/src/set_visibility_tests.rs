#![expect(
    clippy::cast_precision_loss,
    reason = "the fixtures' chunk numbers are small whole numbers, exact in f32"
)]
use super::{SetShape, level_for, visible_runs};
use crate::frustum::Frustum;
use crate::prepare::Batch;
use crate::set_chunks::Chunk;
use bevy_asset::AssetId;
use bevy_asset::uuid::Uuid;
use bevy_ecs::entity::Entity;
use bevy_math::{Mat4, Vec3, proj};
use bevy_mesh::Mesh;

fn mesh(n: u128) -> AssetId<Mesh> {
    AssetId::Uuid {
        uuid: Uuid::from_u128(n),
    }
}

/// A camera at the origin looking down -z, 90 degrees across, reverse-Z infinite (the engine's cameras).
fn camera() -> Frustum {
    Frustum::from_clip_from_world(proj::perspective_infinite_reverse(
        std::f32::consts::FRAC_PI_2,
        1.0,
        0.1,
    ))
}

/// `count` chunks of 10 copies each, in a line along -z starting 10 m ahead, 10 m apart, each 1 m across.
fn line(count: u32) -> Vec<Chunk> {
    (0..count)
        .map(|n| Chunk {
            centre: Vec3::new(0.0, 0.0, -10.0 - 10.0 * n as f32),
            radius: 1.0,
            range: n * 10..(n + 1) * 10,
        })
        .collect()
}

fn run(
    chunks: &[Chunk],
    levels: &[(AssetId<Mesh>, f32)],
    frusta: &[Frustum],
) -> (Vec<Batch>, (u32, u32)) {
    let shape = SetShape {
        chunks,
        levels,
        flags: (false, true, true),
    };
    let mut out = Vec::new();
    let kept = visible_runs(&shape, Entity::PLACEHOLDER, frusta, Vec3::ZERO, &mut out);
    (out, kept)
}

#[test]
fn a_level_is_chosen_by_distance_and_none_is_drawn_past_the_last() {
    let levels = [(mesh(1), 50.0), (mesh(2), 200.0), (mesh(3), 800.0)];
    assert_eq!(level_for(&levels, 0.0), Some(0));
    assert_eq!(level_for(&levels, 50.0), Some(0), "the reach is inclusive");
    assert_eq!(level_for(&levels, 50.1), Some(1));
    assert_eq!(level_for(&levels, 500.0), Some(2));
    assert_eq!(
        level_for(&levels, 800.1),
        None,
        "past the last level nothing is drawn"
    );
    assert_eq!(level_for(&[(mesh(1), f32::INFINITY)], 1.0e9), Some(0));
    assert_eq!(level_for(&[], 1.0), None);
}

#[test]
fn consecutive_visible_chunks_make_one_draw() {
    let chunks = line(8);
    let (batches, (kept, copies)) = run(&chunks, &[(mesh(1), f32::INFINITY)], &[camera()]);
    assert_eq!(
        batches.len(),
        1,
        "all eight are ahead of the camera and adjacent"
    );
    assert_eq!(batches[0].instances, 0..80);
    assert_eq!((kept, copies), (8, 80));
    let batch = &batches[0];
    assert!(batch.set.is_some() && !batch.transparent);
    assert!(
        !batch.outlined && batch.cutout && batch.foliage,
        "the set's flags ride on the batch"
    );
}

#[test]
fn chunks_outside_the_view_are_dropped_and_split_the_runs() {
    let mut chunks = line(6);
    // The third chunk is far off to the side; the rest are ahead.
    chunks[2].centre = Vec3::new(500.0, 0.0, -30.0);
    let (batches, (kept, copies)) = run(&chunks, &[(mesh(1), f32::INFINITY)], &[camera()]);
    assert_eq!(
        batches
            .iter()
            .map(|b| b.instances.clone())
            .collect::<Vec<_>>(),
        vec![0..20, 30..60]
    );
    assert_eq!((kept, copies), (5, 50));
    // Behind the camera is dropped too.
    let mut behind = line(3);
    behind[1].centre = Vec3::new(0.0, 0.0, 40.0);
    let (batches, _) = run(&behind, &[(mesh(1), f32::INFINITY)], &[camera()]);
    assert_eq!(
        batches
            .iter()
            .map(|b| b.instances.clone())
            .collect::<Vec<_>>(),
        vec![0..10, 20..30]
    );
}

#[test]
fn a_chunk_seen_by_either_eye_is_kept() {
    // Two frusta (stereo): one looking down -z and one looking down +x. A chunk ahead on +x is seen by the second only.
    let right = Frustum::from_clip_from_world(
        proj::perspective_infinite_reverse(std::f32::consts::FRAC_PI_2, 1.0, 0.1)
            * Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2),
    );
    let chunks = vec![
        Chunk {
            centre: Vec3::new(0.0, 0.0, -20.0),
            radius: 1.0,
            range: 0..10,
        },
        Chunk {
            centre: Vec3::new(20.0, 0.0, 0.0),
            radius: 1.0,
            range: 10..20,
        },
        Chunk {
            centre: Vec3::new(0.0, 0.0, 20.0),
            radius: 1.0,
            range: 20..30,
        },
    ];
    let levels = [(mesh(1), f32::INFINITY)];
    let (one, _) = run(&chunks, &levels, &[camera()]);
    assert_eq!(
        one.iter().map(|b| b.instances.clone()).collect::<Vec<_>>(),
        vec![0..10]
    );
    let (both, (kept, _)) = run(&chunks, &levels, &[camera(), right]);
    assert_eq!(
        both.iter().map(|b| b.instances.clone()).collect::<Vec<_>>(),
        vec![0..20],
        "adjacent kept chunks merge"
    );
    assert_eq!(kept, 2);
}

#[test]
fn levels_split_a_run_and_far_chunks_beyond_the_last_level_are_not_drawn() {
    // Chunks at 10, 20, ... 80 m (the level distance is the centre's, less half the 1 m radius).
    let chunks = line(8);
    let levels = [(mesh(1), 25.0), (mesh(2), 55.0), (mesh(3), 65.0)];
    let (batches, (kept, _)) = run(&chunks, &levels, &[camera()]);
    let summary: Vec<_> = batches
        .iter()
        .map(|b| (b.mesh, b.instances.clone()))
        .collect();
    assert_eq!(
        summary,
        vec![(mesh(1), 0..20), (mesh(2), 20..50), (mesh(3), 50..60)],
        "near chunks use the first level, farther ones the next, and 70 m and 80 m are past the last"
    );
    assert_eq!(kept, 6);
}

#[test]
fn a_run_never_joins_chunks_that_are_not_adjacent_in_the_buffer() {
    // Two visible chunks with a gap in their ranges (as if one between them had been dropped earlier) stay two draws.
    let chunks = vec![
        Chunk {
            centre: Vec3::new(0.0, 0.0, -10.0),
            radius: 1.0,
            range: 0..10,
        },
        Chunk {
            centre: Vec3::new(0.0, 0.0, -20.0),
            radius: 1.0,
            range: 15..25,
        },
    ];
    let (batches, _) = run(&chunks, &[(mesh(1), f32::INFINITY)], &[camera()]);
    assert_eq!(batches.len(), 2);
}

#[test]
fn nothing_visible_means_no_draws_and_empty_inputs_are_fine() {
    let (batches, kept) = run(&line(4), &[(mesh(1), f32::INFINITY)], &[]);
    assert!(batches.is_empty());
    assert_eq!(kept, (0, 0));
    let (batches, kept) = run(&[], &[(mesh(1), f32::INFINITY)], &[camera()]);
    assert!(batches.is_empty() && kept == (0, 0));
}
