//! Contract tests for ve-nav step N2a (ADR 0063): **links** (jumps, ladders, doors, teleports, one-way, closable) added on top of any space,
//! and a **dynamic overlay** (obstacles and cost volumes that appear and disappear without a rebuild, with a version number and a way to
//! ask whether a path is still good). Both are wrappers that are themselves spaces, so they stack on a graph, a grid or each other.
#![expect(
    clippy::expect_used,
    clippy::cast_possible_truncation,
    reason = "tests build small spaces and assert by failing"
)]

use ve_nav::{
    AreaId, CostFilter, DynamicSpace, Edge, EdgeKind, Link, LinkEnd, LinkState, LinkedSpace,
    NavConfig, NavError, NavMesh, NavSpace, NodeId, SearchLimit, find_node_path, find_point_path,
};

struct Graph {
    pos: Vec<[f32; 3]>,
    edges: Vec<Vec<Edge>>,
}

impl Graph {
    fn new(points: &[[f32; 3]]) -> Self {
        Self {
            pos: points.to_vec(),
            edges: vec![Vec::new(); points.len()],
        }
    }

    fn walk(&mut self, a: usize, b: usize) {
        let length = {
            let (p, q) = (self.pos[a], self.pos[b]);
            ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
        };
        for (from, to) in [(a, b), (b, a)] {
            self.edges[from].push(Edge {
                to: to as NodeId,
                length,
                kind: EdgeKind::WALK,
            });
        }
    }
}

impl NavSpace for Graph {
    fn node_count(&self) -> usize {
        self.pos.len()
    }
    fn position(&self, node: NodeId) -> [f32; 3] {
        self.pos[node as usize]
    }
    fn area(&self, _node: NodeId) -> AreaId {
        AreaId::GROUND
    }
    fn neighbours(&self, node: NodeId, out: &mut Vec<Edge>) {
        out.extend_from_slice(&self.edges[node as usize]);
    }
    fn locate(&self, point: [f32; 3]) -> Option<NodeId> {
        (0..self.pos.len())
            .min_by(|&a, &b| {
                let d = |i: usize| {
                    (self.pos[i][0] - point[0]).powi(2) + (self.pos[i][2] - point[2]).powi(2)
                };
                d(a).total_cmp(&d(b))
            })
            .map(|n| n as NodeId)
    }
}

/// 0 at (0,0,0), 1 at (1,0,1), 2 at (1,0,-3), 3 at (2,0,0): the short way is 0-1-3 (2.83), the long way 0-2-3 (6.32).
fn diamond() -> Graph {
    let mut g = Graph::new(&[
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 1.0],
        [1.0, 0.0, -3.0],
        [2.0, 0.0, 0.0],
    ]);
    g.walk(0, 1);
    g.walk(1, 3);
    g.walk(0, 2);
    g.walk(2, 3);
    g
}

fn route(
    space: &dyn NavSpace,
    filter: &CostFilter,
    from: NodeId,
    to: NodeId,
) -> Result<Vec<NodeId>, NavError> {
    find_node_path(space, filter, from, to, SearchLimit::default()).map(|p| p.nodes)
}

fn link(from: NodeId, to: NodeId, kind: EdgeKind) -> Link {
    Link {
        from: LinkEnd::Node(from),
        to: LinkEnd::Node(to),
        kind,
        length: None,
        one_way: false,
        tag: String::new(),
    }
}

// ---- links ----

#[test]
fn a_jump_link_is_used_only_by_agents_that_can_jump() {
    let mut space = LinkedSpace::new(diamond());
    space.add_link(link(0, 3, EdgeKind::JUMP)).expect("adds");
    let walker = CostFilter::new();
    assert_eq!(route(&space, &walker, 0, 3).expect("route"), vec![0, 1, 3]);
    let mut jumper = CostFilter::new();
    jumper.allow_kind(EdgeKind::JUMP, 1.0);
    assert_eq!(
        route(&space, &jumper, 0, 3).expect("route"),
        vec![0, 3],
        "the 2 m jump beats the 2.83 m walk"
    );
}

#[test]
fn a_link_length_defaults_to_the_distance_and_can_be_set() {
    let mut space = LinkedSpace::new(diamond());
    let mut slow = link(0, 3, EdgeKind::JUMP);
    slow.length = Some(50.0);
    space.add_link(slow).expect("adds");
    let mut jumper = CostFilter::new();
    jumper.allow_kind(EdgeKind::JUMP, 1.0);
    assert_eq!(
        route(&space, &jumper, 0, 3).expect("route"),
        vec![0, 1, 3],
        "a 50 m climb is not worth it"
    );
    let mut edges = Vec::new();
    space.neighbours(0, &mut edges);
    assert!(
        edges
            .iter()
            .any(|e| e.to == 3 && e.kind == EdgeKind::JUMP && (e.length - 50.0).abs() < 1e-6)
    );
}

#[test]
fn one_way_links_only_go_one_way() {
    let mut space = LinkedSpace::new(diamond());
    let mut drop_down = link(3, 0, EdgeKind::JUMP);
    drop_down.one_way = true;
    space.add_link(drop_down).expect("adds");
    let mut jumper = CostFilter::new();
    jumper.allow_kind(EdgeKind::JUMP, 1.0);
    assert_eq!(
        route(&space, &jumper, 3, 0).expect("route"),
        vec![3, 0],
        "down is allowed"
    );
    assert_eq!(
        route(&space, &jumper, 0, 3).expect("route"),
        vec![0, 1, 3],
        "up is not"
    );
}

#[test]
fn a_door_can_be_closed_and_opened_and_only_door_users_pass_it() {
    // A corridor 0-1 and a door link 1-2 with no other way through.
    let mut g = Graph::new(&[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]);
    g.walk(0, 1);
    let mut space = LinkedSpace::new(g);
    let mut door = link(1, 2, EdgeKind::DOOR);
    door.tag = "front_door".into();
    let id = space.add_link(door).expect("adds");
    assert_eq!(space.link_by_tag("front_door"), Some(id));
    let walker = CostFilter::new();
    assert!(
        matches!(route(&space, &walker, 0, 2), Err(NavError::NoRoute { .. })),
        "a plain walker cannot open doors"
    );
    let mut opener = CostFilter::new();
    opener.allow_kind(EdgeKind::DOOR, 1.0);
    assert_eq!(route(&space, &opener, 0, 2).expect("route"), vec![0, 1, 2]);
    let before = space.version();
    space.set_link_state(id, LinkState::Closed).expect("closes");
    assert!(space.version() > before, "every change bumps the version");
    assert_eq!(space.link_state(id), Some(LinkState::Closed));
    assert!(
        matches!(route(&space, &opener, 0, 2), Err(NavError::NoRoute { .. })),
        "a locked door blocks even those who open doors"
    );
    space.set_link_state(id, LinkState::Open).expect("opens");
    assert_eq!(route(&space, &opener, 0, 2).expect("route"), vec![0, 1, 2]);
}

#[test]
fn links_can_be_given_as_points_and_unknown_ones_are_errors() {
    let mut space = LinkedSpace::new(diamond());
    let ladder = Link {
        from: LinkEnd::Point([0.1, 0.0, 0.1]),
        to: LinkEnd::Point([1.9, 0.0, 0.1]),
        kind: EdgeKind::CLIMB,
        length: None,
        one_way: false,
        tag: "ladder".into(),
    };
    space.add_link(ladder).expect("snaps to nodes 0 and 3");
    let mut climber = CostFilter::new();
    climber.allow_kind(EdgeKind::CLIMB, 1.0);
    assert_eq!(route(&space, &climber, 0, 3).expect("route"), vec![0, 3]);
    assert!(
        space
            .set_link_state(ve_nav::LinkId(99), LinkState::Closed)
            .is_err()
    );
    assert!(
        space.add_link(link(0, 99, EdgeKind::JUMP)).is_err(),
        "node 99 does not exist"
    );
}

#[test]
fn a_linked_space_can_be_searched_as_a_trait_object_and_exposes_its_inner_space() {
    let space = LinkedSpace::new(diamond());
    let object: &dyn NavSpace = &space;
    assert_eq!(object.node_count(), 4);
    assert_eq!(space.inner().node_count(), 4);
}

// ---- overlay ----

#[test]
fn an_obstacle_blocks_the_nodes_inside_it_until_it_is_removed() {
    let mut space = DynamicSpace::new(diamond());
    let v0 = space.version();
    let crate_ = space.block_box([0.5, -1.0, 0.5], [1.5, 1.0, 1.5]);
    assert!(space.version() > v0);
    assert_eq!(
        route(&space, &CostFilter::new(), 0, 3).expect("route"),
        vec![0, 2, 3],
        "node 1 is inside the crate"
    );
    assert!(space.remove_obstacle(crate_));
    assert!(!space.remove_obstacle(crate_), "removing twice says so");
    assert_eq!(
        route(&space, &CostFilter::new(), 0, 3).expect("route"),
        vec![0, 1, 3]
    );
}

#[test]
fn a_circle_obstacle_works_in_the_ground_plane() {
    let mut space = DynamicSpace::new(diamond());
    space.block_circle([1.0, 5.0, 1.0], 0.3);
    assert_eq!(
        route(&space, &CostFilter::new(), 0, 3).expect("route"),
        vec![0, 2, 3],
        "height does not matter for a circle"
    );
}

#[test]
fn a_cost_volume_makes_crossing_it_dearer_without_forbidding_it() {
    let mut space = DynamicSpace::new(diamond());
    let fire = space.add_cost_volume([0.5, -1.0, 0.5], [1.5, 1.0, 1.5], 10.0);
    assert_eq!(
        route(&space, &CostFilter::new(), 0, 3).expect("route"),
        vec![0, 2, 3],
        "10x through the fire: the detour wins"
    );
    assert!(space.remove_cost_volume(fire));
    let mut mild = DynamicSpace::new(diamond());
    mild.add_cost_volume([0.5, -1.0, 0.5], [1.5, 1.0, 1.5], 1.5);
    assert_eq!(
        route(&mild, &CostFilter::new(), 0, 3).expect("route"),
        vec![0, 1, 3],
        "at 1.5x the short way still wins"
    );
}

#[test]
fn a_path_can_be_asked_whether_it_is_still_good() {
    let mut space = DynamicSpace::new(diamond());
    let path =
        find_node_path(&space, &CostFilter::new(), 0, 3, SearchLimit::default()).expect("route");
    assert!(space.path_is_valid(&path));
    let crate_ = space.block_box([0.5, -1.0, 0.5], [1.5, 1.0, 1.5]);
    assert!(
        !space.path_is_valid(&path),
        "the path now runs through the obstacle: time to repath"
    );
    space.remove_obstacle(crate_);
    assert!(space.path_is_valid(&path));
}

#[test]
fn overlay_and_links_stack_in_either_order() {
    let mut linked = LinkedSpace::new(diamond());
    linked.add_link(link(0, 3, EdgeKind::JUMP)).expect("adds");
    let mut stacked = DynamicSpace::new(linked);
    let mut jumper = CostFilter::new();
    jumper.allow_kind(EdgeKind::JUMP, 1.0);
    assert_eq!(route(&stacked, &jumper, 0, 3).expect("route"), vec![0, 3]);
    stacked.block_circle([2.0, 0.0, 0.0], 0.2);
    assert!(
        matches!(
            route(&stacked, &jumper, 0, 3),
            Err(NavError::NoRoute { .. })
        ),
        "the goal itself is under the obstacle"
    );
    let blocked_goal = route(&stacked, &jumper, 0, 1).expect("node 1 is free");
    assert_eq!(blocked_goal, vec![0, 1]);
}

fn floor() -> NavMesh {
    let vertices = [
        [0.0, 0.0, 0.0],
        [10.0, 0.0, 0.0],
        [10.0, 0.0, 10.0],
        [0.0, 0.0, 10.0],
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
    NavMesh::build(&vertices, &[0, 1, 2, 0, 2, 3], &config).expect("mesh")
}

#[test]
fn the_overlay_sits_on_the_walkable_grid_too() {
    let mut space = DynamicSpace::new(floor());
    let straight = find_point_path(
        &space,
        &CostFilter::new(),
        [1.0, 0.0, 1.0],
        [9.0, 0.0, 1.0],
        SearchLimit::default(),
    )
    .expect("route");
    assert!(straight.iter().all(|p| p[2] < 7.5), "{straight:?}");
    space.block_box([4.0, -1.0, 0.0], [6.0, 3.0, 8.0]);
    let around = find_point_path(
        &space,
        &CostFilter::new(),
        [1.0, 0.0, 1.0],
        [9.0, 0.0, 1.0],
        SearchLimit::default(),
    )
    .expect("route");
    assert!(
        around.iter().any(|p| p[2] >= 7.5),
        "detours through the free strip: {around:?}"
    );
}
