//! The APIC liquid: it rests when it should, keeps its volume, spreads like a dam break, leaves the flow incompressible, goes round colliders,
//! is deterministic, and refuses what it cannot hold.
#![expect(clippy::unwrap_used, reason = "fixtures")]
use ve_world::cloth::Collider;
use ve_world::fluid::{Apic, Liquid, MAX_APIC_PARTICLES};

const BOX: ([f32; 3], [f32; 3]) = ([0.0, 0.0, 0.0], [1.0, 1.0, 0.4]);

fn tank(min: [f32; 3], max: [f32; 3]) -> Apic {
    Apic::block(min, max, 0.025, Some(BOX)).unwrap()
}

fn run(liquid: &mut Apic, seconds: f32) {
    for _ in 0..(seconds * 60.0) as usize {
        assert!(liquid.step(1.0 / 60.0), "the solver stayed finite");
    }
}

fn speed(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

#[test]
fn a_still_pool_stays_still_and_level() {
    let mut pool = tank([0.0, 0.0, 0.0], [1.0, 0.3, 0.4]);
    let count = pool.pos.len();
    run(&mut pool, 2.0);
    assert_eq!(pool.pos.len(), count);
    let mean = pool.pos.iter().map(|p| p[1]).sum::<f32>() / count as f32;
    assert!(
        (mean - 0.15).abs() < 0.02,
        "the pool kept its depth: mean height {mean}, expected about 0.15"
    );
    let top = pool.pos.iter().map(|p| p[1]).fold(0.0, f32::max);
    assert!(top < 0.34, "no surface spikes: highest particle {top}");
    assert!(
        pool.kinetic_energy() < 0.01,
        "it is at rest: {} J/kg",
        pool.kinetic_energy()
    );
}

#[test]
fn a_dam_break_spreads_along_the_floor_and_settles_inside_the_tank() {
    let mut water = tank([0.0, 0.0, 0.0], [0.4, 0.5, 0.4]);
    let count = water.pos.len();
    run(&mut water, 0.5);
    let early_reach = water.pos.iter().map(|p| p[0]).fold(0.0, f32::max);
    assert!(
        early_reach > 0.55 && early_reach < 1.001,
        "the front is running along the floor after half a second: {early_reach}"
    );
    run(&mut water, 3.0);
    assert_eq!(water.pos.len(), count);
    for p in &water.pos {
        assert!(
            p[0] > -0.001
                && p[0] < 1.001
                && p[1] > -0.001
                && p[1] < 1.001
                && p[2] > -0.001
                && p[2] < 0.401,
            "escaped the tank: {p:?}"
        );
    }
    let reach = water.pos.iter().map(|p| p[0]).fold(0.0, f32::max);
    assert!(reach > 0.95, "it covered the floor: {reach}");
    // the water kept its volume: 0.4 x 0.5 x 0.4 over a 1 x 0.4 floor settles to 0.2 deep
    let mean = water.pos.iter().map(|p| p[1]).sum::<f32>() / count as f32;
    assert!(
        (mean - 0.10).abs() < 0.035,
        "settled depth about 0.2, so the mean height is about 0.1: {mean}"
    );
    let fastest = water.vel.iter().map(|v| speed(*v)).fold(0.0, f32::max);
    assert!(fastest < 1.2, "it is mostly at rest: fastest {fastest} m/s");
}

#[test]
fn the_pressure_projection_removes_the_divergence() {
    let mut water = tank([0.0, 0.0, 0.0], [0.4, 0.5, 0.4]);
    run(&mut water, 0.4);
    let (before, after) = water.last_divergence();
    assert!(before > 0.5, "the flow had divergence to remove: {before}");
    assert!(
        after < before * 0.5,
        "projection cut it: {before} to {after}"
    );
    assert!(after < 12.0, "what is left is small: {after} per second");
}

#[test]
fn it_does_not_gain_energy_from_nothing() {
    let mut pool = tank([0.0, 0.0, 0.0], [1.0, 0.2, 0.4]);
    let mut peak = 0.0_f32;
    for _ in 0..180 {
        assert!(pool.step(1.0 / 60.0));
        peak = peak.max(pool.kinetic_energy());
    }
    assert!(peak < 0.05, "a pool at rest never stirs: peak {peak} J/kg");
}

#[test]
fn a_capsule_pushes_the_water_and_nothing_stays_inside_it() {
    let mut water = tank([0.0, 0.0, 0.0], [1.0, 0.3, 0.4]);
    // a post standing in the middle of the pool
    let (a, b, radius) = ([0.5, 0.0, 0.2], [0.5, 0.5, 0.2], 0.06);
    run(&mut water, 0.3);
    for _ in 0..90 {
        water.colliders_mut().clear();
        water
            .colliders_mut()
            .push(Collider::Capsule { a, b, radius });
        assert!(water.step(1.0 / 60.0));
    }
    let inside = water
        .pos
        .iter()
        .filter(|p| {
            let (d, _) = Collider::Capsule { a, b, radius }.distance(**p);
            d < -0.012
        })
        .count();
    assert!(inside == 0, "{inside} particles ended up inside the post");
}

#[test]
fn the_same_input_gives_the_same_bits() {
    let build = || {
        let mut w = tank([0.0, 0.0, 0.0], [0.4, 0.4, 0.4]);
        run(&mut w, 0.8);
        w
    };
    let (x, y) = (build(), build());
    assert_eq!(x.pos, y.pos);
    assert_eq!(x.vel, y.vel);
}

#[test]
fn a_block_that_cannot_be_held_is_refused() {
    assert!(
        Apic::block([0.0; 3], [1.0, 1.0, 1.0], 0.0, None).is_none(),
        "no spacing"
    );
    assert!(
        Apic::block([0.0; 3], [1.0, 1.0, 1.0], 0.01, None).is_none(),
        "a million particles is over the cap of {MAX_APIC_PARTICLES}"
    );
    assert!(
        Apic::block([0.0; 3], [f32::NAN, 1.0, 1.0], 0.05, None).is_none(),
        "not finite"
    );
    assert!(
        Apic::block([1.0; 3], [0.0; 3], 0.05, None).is_none(),
        "an empty box"
    );
    let ok = Apic::block([0.0; 3], [0.5, 0.5, 0.5], 0.05, None).unwrap();
    let (dims, _) = ok.domain();
    assert!(
        dims.iter().all(|d| *d >= 2),
        "a domain with room round the liquid: {dims:?}"
    );
}
