//! The band scheduler of ADR 0058 as pure logic: which spatial chunk of an instance set is drawn as real copies (near), merged reduced
//! meshes (mid), merged impostor cards (far) or not at all, from the distance to the eyes, with hysteresis and a shared near budget.
//! Written before the bodies; do not weaken.
use ve_world::bands::{Band, BandConfig, ChunkInfo, Scheduler, summary};

const CONFIG: BandConfig = BandConfig {
    near: 20.0,
    mid: 60.0,
    far: 200.0,
    hysteresis: 0.1,
    near_budget: 1_000,
};

fn chunk(x: f32, copies: usize) -> ChunkInfo {
    ChunkInfo {
        centre: [x, 0.0, 0.0],
        radius: 0.0,
        copies,
    }
}

fn bands_at(distances: &[f32], eye: [f32; 3]) -> Vec<Band> {
    let chunks: Vec<_> = distances.iter().map(|d| chunk(*d, 10)).collect();
    Scheduler::new(CONFIG).update(&chunks, &[eye])
}

#[test]
fn distance_picks_the_band() {
    let bands = bands_at(
        &[5.0, 19.0, 21.0, 59.0, 61.0, 199.0, 201.0, 1_000.0],
        [0.0; 3],
    );
    assert_eq!(
        bands,
        vec![
            Band::Near,
            Band::Near,
            Band::Mid,
            Band::Mid,
            Band::Far,
            Band::Far,
            Band::Culled,
            Band::Culled
        ]
    );
}

#[test]
fn a_chunks_radius_brings_it_closer() {
    let big = ChunkInfo {
        centre: [30.0, 0.0, 0.0],
        radius: 15.0,
        copies: 10,
    };
    let bands = Scheduler::new(CONFIG).update(&[big], &[[0.0; 3]]);
    assert_eq!(bands, vec![Band::Near], "its near edge is 15 m away");
}

#[test]
fn the_nearest_of_several_eyes_counts() {
    let chunks = [chunk(100.0, 10)];
    let bands = Scheduler::new(CONFIG).update(&chunks, &[[0.0; 3], [95.0, 0.0, 0.0]]);
    assert_eq!(bands, vec![Band::Near], "the right eye is 5 m away");
}

#[test]
fn no_eyes_means_nothing_is_drawn() {
    let bands = Scheduler::new(CONFIG).update(&[chunk(1.0, 10)], &[]);
    assert_eq!(bands, vec![Band::Culled]);
}

#[test]
fn moving_outward_a_chunk_keeps_its_band_until_it_is_clearly_past_the_edge() {
    let mut scheduler = Scheduler::new(CONFIG);
    let chunks = [chunk(0.0, 10)];
    let at = |scheduler: &mut Scheduler, distance: f32| {
        scheduler.update(&chunks, &[[distance, 0.0, 0.0]])[0]
    };
    assert_eq!(at(&mut scheduler, 15.0), Band::Near);
    assert_eq!(
        at(&mut scheduler, 21.0),
        Band::Near,
        "1 m past the 20 m edge is inside the 10% margin"
    );
    assert_eq!(at(&mut scheduler, 21.9), Band::Near);
    assert_eq!(at(&mut scheduler, 22.1), Band::Mid, "past 22 m it is mid");
    assert_eq!(
        at(&mut scheduler, 21.0),
        Band::Mid,
        "coming back in it does not return until the edge itself"
    );
    assert_eq!(at(&mut scheduler, 19.9), Band::Near);
}

#[test]
fn flicker_on_the_edge_does_not_change_the_band() {
    let mut scheduler = Scheduler::new(CONFIG);
    let chunks = [chunk(0.0, 10)];
    let mut flips = 0;
    let mut last = scheduler.update(&chunks, &[[19.0, 0.0, 0.0]])[0];
    for i in 0..100 {
        let distance = if i % 2 == 0 { 20.5 } else { 19.5 };
        let now = scheduler.update(&chunks, &[[distance, 0.0, 0.0]])[0];
        flips += usize::from(now != last);
        last = now;
    }
    assert_eq!(
        flips, 0,
        "a chunk shivering across the edge by a metre never flips"
    );
}

#[test]
fn the_near_budget_is_shared_nearest_first_and_the_rest_fall_to_mid() {
    // Ten chunks of 100 copies at 2, 4, ... 20 m: the budget of 450 copies allows four near chunks.
    let chunks: Vec<_> = (1..=10_u8)
        .map(|i| chunk(f32::from(i) * 2.0, 100))
        .collect();
    let config = BandConfig {
        near_budget: 450,
        ..CONFIG
    };
    let bands = Scheduler::new(config).update(&chunks, &[[0.0; 3]]);
    let near: Vec<_> = bands
        .iter()
        .enumerate()
        .filter(|(_, b)| **b == Band::Near)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(near, vec![0, 1, 2, 3], "the four nearest");
    assert!(
        bands[4..].iter().all(|b| *b == Band::Mid),
        "the rest of what would be near is drawn merged: {bands:?}"
    );
    let counts = summary(&chunks, &bands);
    assert_eq!(counts.near_copies, 400);
    assert!(counts.near_copies <= 450);
}

#[test]
fn a_chunk_that_alone_exceeds_the_budget_is_never_near() {
    let chunks = [chunk(1.0, 5_000)];
    assert_eq!(
        Scheduler::new(CONFIG).update(&chunks, &[[0.0; 3]]),
        vec![Band::Mid]
    );
}

#[test]
fn the_budget_beats_hysteresis() {
    let mut scheduler = Scheduler::new(BandConfig {
        near_budget: 100,
        ..CONFIG
    });
    let chunks = [chunk(5.0, 100), chunk(6.0, 100)];
    let first = scheduler.update(&chunks, &[[0.0; 3]]);
    assert_eq!(first, vec![Band::Near, Band::Mid]);
    // The eye moves so that the second chunk is nearer: it takes the budget, the first drops even though it was near.
    let second = scheduler.update(&chunks, &[[10.0, 0.0, 0.0]]);
    assert_eq!(second, vec![Band::Mid, Band::Near]);
}

#[test]
fn far_chunks_beyond_the_last_range_are_not_drawn_and_none_is_lost_inside_it() {
    let bands = bands_at(&[150.0, 250.0], [0.0; 3]);
    assert_eq!(bands, vec![Band::Far, Band::Culled]);
}

#[test]
fn the_same_input_gives_the_same_answer_and_ties_go_to_the_lower_index() {
    let chunks = [chunk(5.0, 600), chunk(-5.0, 600)];
    let a = Scheduler::new(CONFIG).update(&chunks, &[[0.0; 3]]);
    let b = Scheduler::new(CONFIG).update(&chunks, &[[0.0; 3]]);
    assert_eq!(a, b);
    assert_eq!(
        a,
        vec![Band::Near, Band::Mid],
        "equal distance: the first one gets the budget"
    );
}

#[test]
fn the_chunk_list_may_grow_and_shrink_between_updates() {
    let mut scheduler = Scheduler::new(CONFIG);
    assert_eq!(scheduler.update(&[], &[[0.0; 3]]), vec![]);
    assert_eq!(
        scheduler.update(&[chunk(5.0, 10), chunk(500.0, 10)], &[[0.0; 3]]),
        vec![Band::Near, Band::Culled]
    );
    assert_eq!(
        scheduler.update(&[chunk(5.0, 10)], &[[0.0; 3]]),
        vec![Band::Near]
    );
}

#[test]
fn the_summary_counts_what_each_band_costs() {
    let chunks = [
        chunk(5.0, 30),
        chunk(40.0, 50),
        chunk(100.0, 70),
        chunk(900.0, 90),
    ];
    let bands = Scheduler::new(CONFIG).update(&chunks, &[[0.0; 3]]);
    let counts = summary(&chunks, &bands);
    assert_eq!(
        (
            counts.near_copies,
            counts.mid_chunks,
            counts.far_chunks,
            counts.culled_chunks
        ),
        (30, 1, 1, 1)
    );
}
