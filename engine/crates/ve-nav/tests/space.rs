//! Contract tests for the extensible core of `ve-nav` (ADR 0063, step N1): a navigable space is a trait, areas and edge kinds are open ids,
//! a per-agent cost filter decides what may be crossed and at what price, and one generic search runs over any space.
#![expect(
    clippy::expect_used,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "tests build small spaces and assert by failing"
)]

use ve_nav::{
    AreaId, CostFilter, Edge, EdgeKind, NavConfig, NavError, NavMesh, NavSpace, NodeId,
    SearchLimit, find_node_path, find_point_path,
};

/// A hand-made graph: the simplest possible `NavSpace`, to prove the search needs nothing else.
struct Graph {
    pos: Vec<[f32; 3]>,
    area: Vec<AreaId>,
    edges: Vec<Vec<Edge>>,
}

impl Graph {
    fn new(points: &[[f32; 3]]) -> Self {
        Self {
            pos: points.to_vec(),
            area: vec![AreaId::GROUND; points.len()],
            edges: vec![Vec::new(); points.len()],
        }
    }

    fn distance(&self, a: usize, b: usize) -> f32 {
        let (p, q) = (self.pos[a], self.pos[b]);
        ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
    }

    fn link(&mut self, a: usize, b: usize, kind: EdgeKind) {
        let length = self.distance(a, b);
        self.edges[a].push(Edge {
            to: b as NodeId,
            length,
            kind,
        });
        self.edges[b].push(Edge {
            to: a as NodeId,
            length,
            kind,
        });
    }
}

impl NavSpace for Graph {
    fn node_count(&self) -> usize {
        self.pos.len()
    }

    fn position(&self, node: NodeId) -> [f32; 3] {
        self.pos[node as usize]
    }

    fn area(&self, node: NodeId) -> AreaId {
        self.area[node as usize]
    }

    fn neighbours(&self, node: NodeId, out: &mut Vec<Edge>) {
        out.extend_from_slice(&self.edges[node as usize]);
    }

    fn locate(&self, point: [f32; 3]) -> Option<NodeId> {
        (0..self.pos.len())
            .min_by(|&a, &b| {
                let da = (self.pos[a][0] - point[0]).powi(2) + (self.pos[a][2] - point[2]).powi(2);
                let db = (self.pos[b][0] - point[0]).powi(2) + (self.pos[b][2] - point[2]).powi(2);
                da.total_cmp(&db)
            })
            .map(|n| n as NodeId)
    }
}

const WATER: AreaId = AreaId(2);

/// Node 0 at the origin, 3 at (2, 0, 0); route A through node 1 (1, 0, 1), route B through node 2 (1, 0, -3): A is 2.828 long, B is 6.325 long.
fn diamond() -> Graph {
    let mut g = Graph::new(&[
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 1.0],
        [1.0, 0.0, -3.0],
        [2.0, 0.0, 0.0],
    ]);
    g.link(0, 1, EdgeKind::WALK);
    g.link(1, 3, EdgeKind::WALK);
    g.link(0, 2, EdgeKind::WALK);
    g.link(2, 3, EdgeKind::WALK);
    g
}

fn limit() -> SearchLimit {
    SearchLimit::default()
}

#[test]
fn the_search_finds_the_shortest_route_over_any_space() {
    let g = diamond();
    let path = find_node_path(&g, &CostFilter::new(), 0, 3, limit()).expect("a route exists");
    assert_eq!(path.nodes, vec![0, 1, 3]);
    assert!(
        (path.cost - 2.0 * 2.0_f32.sqrt()).abs() < 1e-4,
        "{}",
        path.cost
    );
}

#[test]
fn equal_routes_are_broken_the_same_way_every_time_by_lower_node_id() {
    // A symmetric diamond: nodes 1 and 2 are mirror images, so both routes cost the same.
    let mut g = Graph::new(&[
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 1.0],
        [1.0, 0.0, -1.0],
        [2.0, 0.0, 0.0],
    ]);
    g.link(0, 1, EdgeKind::WALK);
    g.link(1, 3, EdgeKind::WALK);
    g.link(0, 2, EdgeKind::WALK);
    g.link(2, 3, EdgeKind::WALK);
    for _ in 0..50 {
        let path = find_node_path(&g, &CostFilter::new(), 0, 3, limit()).expect("route");
        assert_eq!(path.nodes, vec![0, 1, 3], "ties go to the lower node id");
    }
}

#[test]
fn an_excluded_area_is_routed_around() {
    let mut g = diamond();
    g.area[1] = WATER;
    let wet = find_node_path(&g, &CostFilter::new(), 0, 3, limit()).expect("route");
    assert_eq!(
        wet.nodes,
        vec![0, 1, 3],
        "by default every area may be crossed"
    );
    let mut dry_filter = CostFilter::new();
    dry_filter.exclude_area(WATER);
    let dry = find_node_path(&g, &dry_filter, 0, 3, limit()).expect("route");
    assert_eq!(
        dry.nodes,
        vec![0, 2, 3],
        "a swimmer-less agent walks around the water"
    );
}

#[test]
fn an_area_cost_makes_a_longer_route_cheaper() {
    let mut g = diamond();
    g.area[1] = WATER;
    let mut filter = CostFilter::new();
    filter.set_area_cost(WATER, 10.0);
    let path = find_node_path(&g, &filter, 0, 3, limit()).expect("route");
    assert_eq!(
        path.nodes,
        vec![0, 2, 3],
        "water costs 10x, so the 6.3 m detour wins over 2.8 m of water"
    );
    let mut mild = CostFilter::new();
    mild.set_area_cost(WATER, 1.5);
    let path = find_node_path(&g, &mild, 0, 3, limit()).expect("route");
    assert_eq!(
        path.nodes,
        vec![0, 1, 3],
        "at 1.5x the short route is still cheaper"
    );
}

#[test]
fn an_agent_that_may_only_stand_on_some_areas_can_have_no_route() {
    let mut g = diamond();
    g.area[1] = WATER;
    g.area[2] = WATER;
    let mut filter = CostFilter::new();
    filter.only_areas(&[AreaId::GROUND]);
    let error = find_node_path(&g, &filter, 0, 3, limit()).expect_err("both routes cross water");
    assert!(
        matches!(error, NavError::NoRoute { start: 0, goal: 3 }),
        "{error:?}"
    );
}

#[test]
fn edge_kinds_are_open_and_each_agent_chooses_which_it_can_use() {
    let mut g = diamond();
    g.link(0, 3, EdgeKind::JUMP);
    let walker = CostFilter::new();
    let path = find_node_path(&g, &walker, 0, 3, limit()).expect("route");
    assert_eq!(path.nodes, vec![0, 1, 3], "a walker ignores jump links");
    let mut jumper = CostFilter::new();
    jumper.allow_kind(EdgeKind::JUMP, 1.0);
    let path = find_node_path(&g, &jumper, 0, 3, limit()).expect("route");
    assert_eq!(path.nodes, vec![0, 3], "a jumper takes the direct 2 m link");
    let mut reluctant = CostFilter::new();
    reluctant.allow_kind(EdgeKind::JUMP, 5.0);
    let path = find_node_path(&g, &reluctant, 0, 3, limit()).expect("route");
    assert_eq!(
        path.nodes,
        vec![0, 1, 3],
        "jumping costs 5x (10 > 2.83), so it walks"
    );
    let custom = EdgeKind(40);
    let mut g2 = diamond();
    g2.link(0, 3, custom);
    let mut flexible = CostFilter::new();
    flexible.allow_kind(custom, 1.0);
    let path = find_node_path(&g2, &flexible, 0, 3, limit()).expect("route");
    assert_eq!(
        path.nodes,
        vec![0, 3],
        "any id below 64 is a usable kind, for a game's own traversals"
    );
}

#[test]
fn a_route_to_where_you_stand_is_one_node() {
    let g = diamond();
    let path = find_node_path(&g, &CostFilter::new(), 2, 2, limit()).expect("route");
    assert_eq!(path.nodes, vec![2]);
    assert!(path.cost.abs() < 1e-6);
}

#[test]
fn an_unknown_node_is_an_error_not_a_panic() {
    let g = diamond();
    let error = find_node_path(&g, &CostFilter::new(), 0, 99, limit()).expect_err("no node 99");
    assert!(matches!(error, NavError::UnknownNode(99)), "{error:?}");
}

#[test]
fn the_search_stops_at_its_expansion_budget() {
    let n = 100;
    let points: Vec<[f32; 3]> = (0..n).map(|i| [i as f32, 0.0, 0.0]).collect();
    let mut g = Graph::new(&points);
    for i in 0..n - 1 {
        g.link(i, i + 1, EdgeKind::WALK);
    }
    let error = find_node_path(
        &g,
        &CostFilter::new(),
        0,
        (n - 1) as NodeId,
        SearchLimit { max_expansions: 10 },
    )
    .expect_err("a 99-hop chain cannot be searched in 10 expansions");
    assert!(
        matches!(error, NavError::SearchLimit { expanded } if expanded == 10),
        "{error:?}"
    );
    let path = find_node_path(
        &g,
        &CostFilter::new(),
        0,
        (n - 1) as NodeId,
        SearchLimit {
            max_expansions: 1000,
        },
    )
    .expect("enough budget");
    assert_eq!(path.nodes.len(), n);
    assert!((path.cost - 99.0).abs() < 1e-3);
    assert!(path.expanded <= 100);
}

#[test]
fn a_space_can_be_used_as_a_trait_object() {
    let g = diamond();
    let space: &dyn NavSpace = &g;
    let path = find_node_path(space, &CostFilter::new(), 0, 3, limit()).expect("route");
    assert_eq!(path.nodes, vec![0, 1, 3]);
}

#[test]
fn points_are_snapped_to_the_space_and_come_back_as_positions() {
    let g = diamond();
    let points = find_point_path(
        &g,
        &CostFilter::new(),
        [0.1, 0.0, 0.1],
        [2.2, 0.0, -0.1],
        limit(),
    )
    .expect("route");
    assert_eq!(
        points,
        vec![[0.0, 0.0, 0.0], [1.0, 0.0, 1.0], [2.0, 0.0, 0.0]]
    );
}

#[test]
fn an_empty_filter_default_allows_every_area_and_only_walking() {
    let filter = CostFilter::new();
    assert!(filter.allows_area(AreaId::GROUND));
    assert!(filter.allows_area(AreaId(63)));
    assert!(filter.allows_kind(EdgeKind::WALK));
    assert!(!filter.allows_kind(EdgeKind::JUMP));
    assert!((filter.area_cost(AreaId(7)) - 1.0).abs() < 1e-6);
}

#[test]
fn area_ids_above_the_limit_are_refused() {
    assert!(AreaId::new(63).is_some());
    assert!(AreaId::new(64).is_none());
}

// ---- the existing walkable grid is just another space ----

fn floor_mesh() -> NavMesh {
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
fn the_walkable_grid_is_a_nav_space_with_a_real_path() {
    let nav = floor_mesh();
    let path = find_point_path(
        &nav,
        &CostFilter::new(),
        [1.0, 0.0, 1.0],
        [9.0, 0.0, 1.0],
        limit(),
    )
    .expect("route");
    assert!(path.len() >= 2);
    let first = path[0];
    let last = *path.last().expect("points");
    assert!(
        (first[0] - 1.0).abs() <= 0.5 && (first[2] - 1.0).abs() <= 0.5,
        "{first:?}"
    );
    assert!(
        (last[0] - 9.0).abs() <= 0.5 && (last[2] - 1.0).abs() <= 0.5,
        "{last:?}"
    );
}

#[test]
fn painting_an_area_on_the_grid_steers_agents_around_it() {
    let mut nav = floor_mesh();
    // A 4 m wide band of water across the whole depth would block everything, so leave a dry strip at z in [8, 10].
    nav.paint_area([4.0, -1.0, 0.0], [6.0, 3.0, 8.0], WATER);
    let mut filter = CostFilter::new();
    filter.exclude_area(WATER);
    let dry = find_point_path(&nav, &filter, [1.0, 0.0, 1.0], [9.0, 0.0, 1.0], limit())
        .expect("route around");
    assert!(
        dry.iter().any(|p| p[2] >= 7.5),
        "the dry route detours through the strip: {dry:?}"
    );
    let wet = find_point_path(
        &nav,
        &CostFilter::new(),
        [1.0, 0.0, 1.0],
        [9.0, 0.0, 1.0],
        limit(),
    )
    .expect("route");
    assert!(
        wet.iter().all(|p| p[2] < 7.5),
        "with no filter the straight way through the water is taken: {wet:?}"
    );
}
