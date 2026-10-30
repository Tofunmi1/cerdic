//! Creatures that move on their own (ADR 0067): a kind declared with an `agent` line patrols a loop of points, notices the hero when it is
//! within `sight` metres and in view, chases at a faster speed, can be stunned (by a rule: `stun:SELECTOR:SECONDS`), and raises events a rule
//! can use (`sight`, `lost`, `stun`, `recover`, `catch`). It walks the walkable grid of a `ve_nav::NavMesh` the host gives the world
//! (`World::set_nav`), so it goes round a hole or a wall; without one it walks straight. Everything is deterministic at the fixed 1/60 s step.
//! Written before the body; do not weaken.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use std::sync::Arc;

use ve_game::{EffectKind, GameDef, Inputs, World};
use ve_nav::{NavConfig, NavMesh};

/// A flat floor of 1 m cells over [-10, 10] x [-10, 10], with the cells inside `hole` left out (a pit or a wall block nobody can cross).
fn floor(hole: Option<([f32; 2], [f32; 2])>) -> Arc<NavMesh> {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for ix in -10..10 {
        for iz in -10..10 {
            let (x, z) = (ix as f32, iz as f32);
            if let Some(([x0, x1], [z0, z1])) = hole {
                if x + 0.5 > x0 && x + 0.5 < x1 && z + 0.5 > z0 && z + 0.5 < z1 {
                    continue;
                }
            }
            let base = vertices.len() as u32;
            vertices.extend([
                [x, 0.0, z],
                [x + 1.0, 0.0, z],
                [x + 1.0, 0.0, z + 1.0],
                [x, 0.0, z + 1.0],
            ]);
            indices.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
        }
    }
    let config = NavConfig::new(
        0.25,
        0.3,
        1.8,
        0.4,
        45.0,
        [-10.5, -1.0, -10.5],
        [10.5, 3.0, 10.5],
    )
    .unwrap();
    Arc::new(NavMesh::build(&vertices, &indices, &config).unwrap())
}

const BASE: &str = "game name=agents
var caught=0
var seen=0
var lost=0
var stunned=0
var recovered=0
kind player size=0.4 tags=hero
kind creature size=0.4 tags=creature part=creature_body
agent creature patrol=-6,-6;6,-6;6,6 speed=1.5 chase_speed=3 sight=5 lose=9
input walk follow=viewer target=tag:hero
rule on=sight do=add:seen:1
rule on=lost do=add:lost:1
rule on=stun do=add:stunned:1
rule on=recover do=add:recovered:1
rule on=catch do=add:caught:1
";

fn world(extra: &str, nav: Option<Arc<NavMesh>>) -> World {
    let mut w = World::new(&GameDef::parse(&format!("{BASE}{extra}")).unwrap(), 1);
    w.set_nav(nav);
    w
}

fn viewer(x: f64, z: f64) -> Inputs {
    Inputs {
        viewer: Some([x, 0.0, z]),
        ..Inputs::default()
    }
}

fn creature(w: &World) -> [f64; 3] {
    w.entities()
        .iter()
        .find(|e| e.kind == "creature")
        .map(|e| e.position)
        .unwrap()
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn run(w: &mut World, steps: usize, at: (f64, f64)) {
    for _ in 0..steps {
        w.step(&viewer(at.0, at.1)).unwrap();
    }
}

#[test]
fn an_agent_line_is_read_and_comes_back_in_the_description() {
    let def = GameDef::parse(&format!("{BASE}spawn creature at=-6,0,-6\n")).unwrap();
    let text = def.describe();
    assert!(text.contains("agent creature"), "{text}");
    assert!(
        text.contains("patrol=-6,-6;6,-6;6,6") && text.contains("sight=5"),
        "{text}"
    );
    assert_eq!(
        GameDef::parse(&text).unwrap().describe(),
        text,
        "and it round trips"
    );
}

#[test]
fn a_bad_agent_line_names_the_problem() {
    for (line, word) in [
        ("agent ghost patrol=1,1 speed=1\n", "ghost"),
        ("agent creature patrol=1 speed=1\n", "patrol"),
        ("agent creature patrol=1,1;2,2 speed=-1\n", "speed"),
        ("agent creature patrol=1,1;2,2 speed=1 sight=far\n", "sight"),
    ] {
        let diagnostics =
            GameDef::parse(&format!("game name=a\nkind creature size=0.4\n{line}")).unwrap_err();
        assert!(
            diagnostics.iter().any(|d| d.message.contains(word)),
            "{line}: {diagnostics:?}"
        );
    }
}

#[test]
fn it_patrols_its_loop_at_walking_speed() {
    let mut w = world(
        "spawn creature at=-6,0,-6\nspawn player at=-9,0,9\n",
        Some(floor(None)),
    );
    run(&mut w, 60, (-9.0, 9.0));
    let after_one_second = creature(&w);
    assert!(
        (distance(after_one_second, [-6.0, 0.0, -6.0]) - 1.5).abs() < 0.3,
        "{after_one_second:?}"
    );
    // 12 m to the second point at 1.5 m/s is 8 s; after 10 s it has turned the corner and is some 3 m up the next leg (x = 6, z from -6 to 6).
    run(&mut w, 540, (-9.0, 9.0));
    let p = creature(&w);
    assert!(
        p[0] > 4.5 && p[2] > -5.0 && p[2] < 0.0,
        "it has turned the first corner and is on the next leg: {p:?}"
    );
}

#[test]
fn it_notices_the_hero_in_sight_and_gives_chase_at_the_faster_speed() {
    let mut w = world(
        "spawn creature at=0,0,0\nspawn player at=4,0,0\n",
        Some(floor(None)),
    );
    run(&mut w, 1, (4.0, 0.0));
    assert!(w.vars()["seen"] >= 1.0, "a sight event: {:?}", w.vars());
    let before = creature(&w);
    run(&mut w, 30, (4.0, 0.0));
    let gained = creature(&w)[0] - before[0];
    assert!(
        gained > 1.2,
        "half a second of chase at 3 m/s: moved {gained} m toward the hero"
    );
}

#[test]
fn it_does_not_notice_a_hero_beyond_its_sight() {
    let mut w = world(
        "spawn creature at=-8,0,-8\nspawn player at=8,0,8\n",
        Some(floor(None)),
    );
    run(&mut w, 60, (8.0, 8.0));
    assert_eq!(w.vars()["seen"], 0.0);
}

#[test]
fn it_catches_the_hero_it_reaches_and_says_so_once() {
    let mut w = world(
        "spawn creature at=0,0,0\nspawn player at=3,0,0\n",
        Some(floor(None)),
    );
    run(&mut w, 240, (3.0, 0.0));
    assert!(w.vars()["caught"] >= 1.0, "{:?}", w.vars());
}

#[test]
fn it_goes_round_a_hole_to_reach_the_hero() {
    // A block of floor missing between the creature and the hero: x in (-1, 1), z in (-1.5, 1.5). The hero is first in view (beside the
    // creature), then jumps to the far side: it keeps chasing for the 3 s it may lose sight for, and must go round the hole (about 7.6 m,
    // 2.5 s at 3 m/s) to get there.
    let nav = floor(Some(([-1.0, 1.0], [-1.5, 1.5])));
    let mut w = world(
        "spawn creature at=-3,0,0\nspawn player at=-3,0,3\n",
        Some(nav),
    );
    for _ in 0..5 {
        w.step(&viewer(-3.0, 3.0)).unwrap();
    }
    assert!(
        w.vars()["seen"] >= 1.0,
        "it saw the hero first: {:?}",
        w.vars()
    );
    let (mut crossed_hole, mut closest) = (false, f64::MAX);
    for _ in 0..170 {
        w.step(&viewer(3.0, 0.0)).unwrap();
        let p = creature(&w);
        // Cells are kept or dropped by their centres, so the hole is really x and z from -1 to 1 (the path skirts it at z = -1).
        if p[0].abs() < 0.8 && p[2].abs() < 0.8 {
            crossed_hole = true;
        }
        closest = closest.min(distance(p, [3.0, 0.0, 0.0]));
    }
    assert!(!crossed_hole, "it walked over the hole");
    assert!(
        closest < 1.5,
        "it got round to the hero: closest {closest} m"
    );
}

#[test]
fn a_stun_rule_freezes_it_for_its_seconds_and_then_it_recovers() {
    let extra =
        "spawn creature at=0,0,0\nspawn player at=4,0,0\nrule on=start do=stun:kind:creature:2\n";
    let mut w = world(extra, Some(floor(None)));
    run(&mut w, 1, (4.0, 0.0));
    let frozen = creature(&w);
    run(&mut w, 100, (4.0, 0.0)); // 1.67 s
    assert!(
        distance(creature(&w), frozen) < 0.05,
        "does not move while stunned"
    );
    assert!(w.vars()["stunned"] >= 1.0);
    run(&mut w, 60, (4.0, 0.0)); // past 2 s
    assert!(w.vars()["recovered"] >= 1.0, "{:?}", w.vars());
    run(&mut w, 30, (4.0, 0.0));
    assert!(
        distance(creature(&w), frozen) > 0.5,
        "and goes after the hero again"
    );
}

#[test]
fn a_stun_can_be_aimed_with_an_animation_trigger() {
    let extra =
        "spawn creature at=0,0,0\nspawn player at=9,0,9\nrule on=stun do=effect:anim:stagger@a\n";
    let mut w = world(
        &format!("{extra}rule on=start do=stun:kind:creature:1\n"),
        Some(floor(None)),
    );
    let outcome = w.step(&viewer(9.0, 9.0)).unwrap();
    assert!(
        outcome.effects.iter().any(|e| e.kind == EffectKind::Anim
            && e.name == "stagger"
            && e.target
                == Some(
                    w.entities()
                        .iter()
                        .find(|x| x.kind == "creature")
                        .unwrap()
                        .id
                )),
        "{:?}",
        outcome.effects
    );
}

#[test]
fn without_a_navmesh_it_still_walks_straight() {
    let mut w = world("spawn creature at=0,0,0\nspawn player at=4,0,0\n", None);
    run(&mut w, 60, (4.0, 0.0));
    assert!(creature(&w)[0] > 1.5, "{:?}", creature(&w));
}

#[test]
fn the_same_chase_gives_the_same_hash() {
    let make = || {
        let mut w = world(
            "spawn creature at=0,0,0\nspawn creature at=-3,0,3\nspawn player at=4,0,0\n",
            Some(floor(Some(([-1.0, 1.0], [-2.0, 2.0])))),
        );
        run(&mut w, 300, (4.0, 0.0));
        w.hash()
    };
    assert_eq!(make(), make());
}

#[test]
fn a_world_without_agents_hashes_as_before() {
    let text = "game name=a\nkind coin size=0.4\nspawn coin at=1,0,0\n";
    let mut with_nav = World::new(&GameDef::parse(text).unwrap(), 1);
    with_nav.set_nav(Some(floor(None)));
    let mut without = World::new(&GameDef::parse(text).unwrap(), 1);
    for _ in 0..30 {
        with_nav.step(&Inputs::default()).unwrap();
        without.step(&Inputs::default()).unwrap();
    }
    assert_eq!(
        with_nav.hash(),
        without.hash(),
        "the navmesh is not part of the state"
    );
}
