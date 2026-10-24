//! Extra deterministic coverage for N2 navigation extensions.
#![expect(
    clippy::expect_used,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    reason = "the local graph fixture uses exact small coordinates"
)]
use ve_nav::{
    AreaId, DynamicSpace, Edge, EdgeKind, Link, LinkEnd, LinkState, LinkedSpace, NavConfig,
    NavError, NavMesh, NavSpace, NodeId, ObjectId, SearchLimit, SlotDef, SmartObjectDef,
    SmartObjects, find_node_path,
};

struct Graph {
    p: Vec<[f32; 3]>,
    e: Vec<Vec<Edge>>,
}
impl Graph {
    fn new(p: &[[f32; 3]]) -> Self {
        Self {
            p: p.to_vec(),
            e: vec![Vec::new(); p.len()],
        }
    }
}
impl NavSpace for Graph {
    fn node_count(&self) -> usize {
        self.p.len()
    }
    fn position(&self, n: NodeId) -> [f32; 3] {
        self.p[n as usize]
    }
    fn area(&self, _: NodeId) -> AreaId {
        AreaId::GROUND
    }
    fn neighbours(&self, n: NodeId, out: &mut Vec<Edge>) {
        out.extend_from_slice(&self.e[n as usize]);
    }
    fn locate(&self, p: [f32; 3]) -> Option<NodeId> {
        self.p
            .iter()
            .position(|q| *q == p)
            .and_then(|i| u32::try_from(i).ok())
    }
}
fn link(a: u32, b: u32, one_way: bool) -> Link {
    Link {
        from: LinkEnd::Node(a),
        to: LinkEnd::Node(b),
        kind: EdgeKind::JUMP,
        length: None,
        one_way,
        tag: "same".into(),
    }
}
fn def(cooldown: f64, ttl: f64, slots: usize) -> SmartObjectDef {
    SmartObjectDef {
        kind: "seat".into(),
        position: [0.0; 3],
        slots: vec![
            SlotDef {
                offset: [0.0; 3],
                facing_deg: 0.0
            };
            slots
        ],
        cooldown_secs: cooldown,
        claim_ttl_secs: ttl,
    }
}

#[test]
fn link_ids_tags_versions_self_edges_and_duplicates_are_stable() {
    let mut s = LinkedSpace::new(Graph::new(&[[0.0; 3], [1.0, 0.0, 0.0]]));
    assert_eq!(s.version(), 0);
    let a = s.add_link(link(0, 1, false)).expect("link");
    let b = s.add_link(link(0, 1, true)).expect("second");
    let c = s.add_link(link(0, 0, false)).expect("self");
    assert_eq!((a.0, b.0, c.0), (0, 1, 2));
    assert_eq!(s.link_by_tag("same"), Some(a));
    assert_eq!(s.version(), 3);
    s.set_link_state(b, LinkState::Closed).expect("close");
    assert_eq!(s.version(), 4);
    s.set_link_state(b, LinkState::Closed).expect("no-op");
    assert_eq!(s.version(), 4);
}
#[test]
fn invalid_node_link_reports_node_error() {
    let mut s = LinkedSpace::new(Graph::new(&[[0.0; 3]]));
    assert!(matches!(
        s.add_link(link(0, 9, true)),
        Err(NavError::UnknownNode(9))
    ));
}
#[test]
fn overlay_box_is_inclusive_and_versions_track_successful_mutations() {
    let mut s = DynamicSpace::new(Graph::new(&[[-1.0, 0.0, 0.0], [0.0; 3], [1.0, 0.0, 0.0]]));
    let id = s.block_box([0.0, -1.0, -1.0], [1.0, 1.0, 1.0]);
    let mut out = Vec::new();
    s.neighbours(0, &mut out);
    assert!(out.is_empty());
    assert_eq!(s.version(), 1);
    assert!(s.remove_obstacle(id));
    assert!(!s.remove_obstacle(id));
    assert_eq!(s.version(), 2);
}
#[test]
fn smart_slots_cool_down_independently_and_expire_strictly_after_ttl() {
    let mut s = SmartObjects::new();
    let id = s.add(def(5.0, 10.0, 2));
    let a = s.claim(id, "a", 0.0).expect("first");
    let b = s.claim(id, "b", 0.0).expect("second");
    s.release(&a, 2.0).expect("release");
    assert_eq!(
        s.claim(id, "c", 3.0).expect_err("first slot cooling"),
        ve_nav::ClaimError::Cooldown
    );
    assert_eq!(
        s.claim(id, "c", 7.0)
            .expect("second slot remains independently available")
            .slot,
        0
    );
    assert!(s.tick(10.0).is_empty());
    assert_eq!(s.tick(10.01).len(), 1);
    assert_eq!(b.slot, 1);
}
#[test]
fn zero_slot_objects_are_full_and_never_found() {
    let mut s = SmartObjects::new();
    let id = s.add(def(0.0, 1.0, 0));
    assert!(s.find("seat", [0.0; 3], 1.0, 0.0).is_empty());
    assert_eq!(
        s.claim(id, "a", 0.0).expect_err("no slots"),
        ve_nav::ClaimError::Full
    );
}
#[test]
fn object_ids_start_at_one() {
    let mut s = SmartObjects::new();
    assert_eq!(s.add(def(0.0, 1.0, 1)), ObjectId(1));
}

fn split_floor() -> NavMesh {
    // Two broad floor islands separated by a full-width, non-walkable gap.
    let vertices = [
        [0.0, 0.0, 0.0],
        [4.0, 0.0, 0.0],
        [4.0, 0.0, 10.0],
        [0.0, 0.0, 10.0],
        [6.0, 0.0, 0.0],
        [10.0, 0.0, 0.0],
        [10.0, 0.0, 10.0],
        [6.0, 0.0, 10.0],
    ];
    let config = NavConfig::new(
        0.5,
        0.0,
        1.8,
        0.4,
        45.0,
        [0.0, -1.0, 0.0],
        [10.0, 3.0, 10.0],
    )
    .expect("config");
    NavMesh::build(&vertices, &[0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7], &config)
        .expect("split floor mesh")
}

#[test]
fn jump_link_crosses_a_full_width_gap_that_walkers_cannot_cross() {
    let mut space = LinkedSpace::new(split_floor());
    let from = space.locate([2.0, 0.0, 5.0]).expect("left floor");
    let to = space.locate([8.0, 0.0, 5.0]).expect("right floor");
    let mut jump = link(from, to, true);
    jump.length = Some(6.0);
    space.add_link(jump).expect("jump link");
    let plain = ve_nav::CostFilter::new();
    assert!(matches!(
        find_node_path(&space, &plain, from, to, SearchLimit::default()),
        Err(NavError::NoRoute { .. })
    ));
    let mut jumper = ve_nav::CostFilter::new();
    jumper.allow_kind(EdgeKind::JUMP, 1.0);
    let path = find_node_path(&space, &jumper, from, to, SearchLimit::default())
        .expect("jump crosses gap");
    assert_eq!(path.nodes, vec![from, to]);
}

#[test]
fn overlapping_cost_volumes_multiply_and_removal_restores_exact_lengths() {
    let mut graph = Graph::new(&[[0.0; 3], [1.0, 0.0, 0.0]]);
    graph.e[0].push(Edge {
        to: 1,
        length: 3.25,
        kind: EdgeKind::WALK,
    });
    let mut space = DynamicSpace::new(graph);
    let mut before = Vec::new();
    space.neighbours(0, &mut before);
    let first = space.add_cost_volume([0.5, -1.0, -1.0], [1.5, 1.0, 1.0], 2.0);
    let second = space.add_cost_volume([0.5, -1.0, -1.0], [1.5, 1.0, 1.0], 3.0);
    let mut weighted = Vec::new();
    space.neighbours(0, &mut weighted);
    assert_eq!(weighted[0].length, before[0].length * 6.0);
    assert!(space.remove_cost_volume(first));
    assert!(space.remove_cost_volume(second));
    let mut restored = Vec::new();
    space.neighbours(0, &mut restored);
    assert_eq!(restored[0].length, before[0].length);
}

#[test]
fn seeded_claim_release_stress_preserves_ownership_and_find_invariants() {
    let mut world = SmartObjects::new();
    let ids: Vec<_> = (0..8)
        .map(|i| {
            let mut object = def(2.0, 10_000.0, 4);
            object.position = [i as f32 * 2.0, 0.0, 0.0];
            world.add(object)
        })
        .collect();
    let mut rng_state = 0x6d2b_79f5_u32;
    let mut now = 0.0;
    for step in 0..5000 {
        rng_state = rng_state
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        let object = ids[(rng_state as usize) % ids.len()];
        let npc = format!("npc-{}", (rng_state >> 8) % 32);
        if rng_state & 1 == 0 {
            if let Ok(claim) = world.claim(object, &npc, now) {
                assert!(
                    !world
                        .claimed_by(&npc)
                        .iter()
                        .any(|held| { held.object == claim.object && held.slot != claim.slot })
                );
            }
        } else {
            let held = world.claimed_by(&npc);
            if let Some(claim) = held.iter().find(|claim| claim.object == object) {
                world.release(claim, now).expect("listed claim releases");
            }
        }
        now += 0.01;

        let mut held_slots = std::collections::BTreeSet::new();
        for index in 0..32 {
            for claim in world.claimed_by(&format!("npc-{index}")) {
                assert!(
                    held_slots.insert((claim.object, claim.slot)),
                    "duplicate held slot"
                );
            }
        }
        let offered = world.find("seat", [0.0, 0.0, 0.0], 100.0, now);
        for offered_id in offered {
            let probe = format!("probe-{step}-{}", offered_id.0);
            let claim = world
                .claim(offered_id, &probe, now)
                .expect("find must not offer full or cooling objects");
            assert_eq!(claim.object, offered_id);
            world.release(&claim, now).expect("probe release");
        }
    }
}
