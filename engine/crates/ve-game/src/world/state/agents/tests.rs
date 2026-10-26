//! Additional contract coverage for rollback, contacts, obstacles and loss of sight.
use super::*;
use crate::{GameDef, Inputs};
use std::collections::BTreeMap;
use ve_nav::{NavConfig, NavMesh};

const SOURCE: &str = "game name=contract
var caught=0 lost=0 recovered=0 stunned=0
kind player size=0.4 tags=hero
kind creature size=0.4
agent creature patrol=-3,0;-3,5 speed=1.5 chase_speed=3 sight=10 lose=20
spawn creature at=-3,0,0
spawn player at=-1,0,0
input walk follow=viewer target=tag:hero
input zap key=press
rule on=input:zap do=stun:kind:creature:0.5
rule on=catch do=add:caught:1
rule on=lost do=add:lost:1
rule on=stun do=add:stunned:1
rule on=recover do=add:recovered:1
";

fn world(extra: &str) -> World {
    World::new(&GameDef::parse(&format!("{SOURCE}{extra}")).unwrap(), 7)
}

fn run(w: &mut World, steps: usize, at: [f64; 3]) {
    for _ in 0..steps {
        w.step(&Inputs {
            viewer: Some(at),
            ..Inputs::default()
        })
        .unwrap();
    }
}

fn mesh(hole: bool) -> Arc<NavMesh> {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for x in -6_i16..6 {
        for z in -6_i16..6 {
            if hole && (-1..1).contains(&x) && (-2..2).contains(&z) {
                continue;
            }
            let base = u32::try_from(vertices.len()).unwrap();
            let (x, z) = (f32::from(x), f32::from(z));
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
        [-6.0, -1.0, -6.0],
        [6.0, 3.0, 6.0],
    )
    .unwrap();
    Arc::new(NavMesh::build(&vertices, &indices, &config).unwrap())
}

#[test]
fn an_acquired_hero_is_reached_around_a_hole_without_crossing_it() {
    let mut w = world("");
    let nav = mesh(true);
    w.set_nav(Some(Arc::clone(&nav)));
    run(&mut w, 2, [-3.0, 0.0, 1.0]);
    assert_eq!(w.agents[&0].mode, Mode::Chase);
    for _ in 0..240 {
        run(&mut w, 1, [3.0, 0.0, 0.0]);
        assert!(
            nav.is_walkable(point(w.entities[0].position)),
            "entity={:?}, state={:?}",
            w.entities[0],
            w.agents[&0]
        );
    }
    assert!(
        distance(w.entities[0].position, [3.0, 0.0, 0.0]) < 0.5,
        "{:?}",
        w.entities[0]
    );
    assert_eq!(w.vars["caught"].to_bits(), 1.0_f64.to_bits());
}

#[test]
fn stun_path_and_contact_state_replay_after_restore() {
    let mut w = world("");
    let nav = mesh(false);
    w.set_nav(Some(Arc::clone(&nav)));
    run(&mut w, 10, [-1.0, 0.0, 0.0]);
    w.step(&Inputs {
        pressed: vec!["zap".into()],
        ..Inputs::default()
    })
    .unwrap();
    let snapshot = w.snapshot();
    run(&mut w, 120, [-1.0, 0.0, 0.0]);
    let hash = w.hash();
    let entities = w.entities.clone();
    assert_eq!(w.vars["recovered"].to_bits(), 1.0_f64.to_bits());
    assert_eq!(w.vars["caught"].to_bits(), 1.0_f64.to_bits());
    w.restore(&snapshot).unwrap();
    assert!(Arc::ptr_eq(w.nav().unwrap(), &nav));
    run(&mut w, 120, [-1.0, 0.0, 0.0]);
    assert_eq!(w.entities, entities);
    assert_eq!(w.hash(), hash);
}

#[test]
fn catches_once_until_separation_or_stun_and_ignores_bad_stun_time() {
    let mut w = world("rule on=start do=stun:kind:creature:garbage\n");
    run(&mut w, 120, [-1.0, 0.0, 0.0]);
    assert_eq!(w.vars["stunned"].to_bits(), 0.0_f64.to_bits());
    assert_eq!(w.vars["caught"].to_bits(), 1.0_f64.to_bits());
    run(&mut w, 1, [4.0, 0.0, 0.0]);
    run(&mut w, 200, [-1.0, 0.0, 0.0]);
    assert_eq!(w.vars["caught"].to_bits(), 2.0_f64.to_bits());
    w.step(&Inputs {
        pressed: vec!["zap".into()],
        ..Inputs::default()
    })
    .unwrap();
    run(&mut w, 120, [-1.0, 0.0, 0.0]);
    assert_eq!(w.vars["caught"].to_bits(), 3.0_f64.to_bits());
}

#[test]
fn a_chase_is_lost_after_three_seconds_out_of_view() {
    let mut w = world("");
    w.set_nav(Some(mesh(true)));
    run(&mut w, 2, [-3.0, 0.0, 1.0]);
    // A solid box holds the creature still, so it cannot walk around the occluder.
    run(&mut w, 1, [3.0, 0.0, 0.0]);
    let kind = w.def.kinds[1].clone();
    let mut wall = kind;
    wall.name = "wall".into();
    wall.solid = Some([1.0, 2.0, 2.0]);
    w.def.kinds.push(wall);
    w.spawn("wall", w.entities[0].position, BTreeMap::default(), 0.0);
    run(&mut w, 179, [3.0, 0.0, 0.0]);
    assert_eq!(w.vars["lost"].to_bits(), 0.0_f64.to_bits()); // viewer movement is applied after the agent decision
    run(&mut w, 1, [3.0, 0.0, 0.0]);
    assert_eq!(
        w.vars["lost"].to_bits(),
        1.0_f64.to_bits(),
        "entities={:?}, state={:?}",
        w.entities,
        w.agents[&0]
    );
    assert_eq!(w.agents[&0].mode, Mode::Patrol);
}

#[test]
fn solids_stop_agents_and_far_heroes_end_chases() {
    let mut w = world("kind wall solid=1,3,6\nspawn wall at=-1.5,0,0\n");
    run(&mut w, 120, [2.0, 0.0, 0.0]);
    assert!(w.entities[0].position[0] <= -2.4 + 1e-7);
    assert!(
        w.entities[0]
            .velocity
            .iter()
            .all(|v| v.abs() < f64::EPSILON)
    );
    run(&mut w, 2, [30.0, 0.0, 0.0]);
    assert_eq!(w.vars["lost"].to_bits(), 1.0_f64.to_bits());
}

#[test]
fn a_mesh_without_agent_declarations_adds_no_hash_or_snapshot_state() {
    let definition =
        GameDef::parse("game name=plain\nkind coin size=0.4\nspawn coin at=1,0,0\n").unwrap();
    let mut w = World::new(&definition, 1);
    assert!(w.snapshot().agents.is_none());
    let hash = w.hash();
    w.set_nav(Some(mesh(false)));
    assert_eq!(w.hash(), hash);
    let mut changed = definition;
    changed
        .agents
        .push(GameDef::parse(SOURCE).unwrap().agents[0].clone());
    let other = World::new(&changed, 1);
    assert!(w.restore(&other.snapshot()).is_err());
}

#[test]
fn replacing_host_navigation_preserves_agent_hash_and_restore_keeps_the_host_mesh() {
    let mut w = world("");
    run(&mut w, 10, [-1.0, 0.0, 0.0]);
    let snapshot = w.snapshot();
    let hash = w.hash();
    let nav = mesh(false);
    w.set_nav(Some(Arc::clone(&nav)));
    assert_eq!(w.hash(), hash);
    run(&mut w, 30, [-1.0, 0.0, 0.0]);
    w.restore(&snapshot).unwrap();
    assert_eq!(w.hash(), hash);
    assert!(Arc::ptr_eq(w.nav().unwrap(), &nav));
}
