//! Behaviour tests for the navigation crate: real levels, real paths, real crowds.
#![expect(
    clippy::expect_used,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "tests build small meshes and assert by failing"
)]

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use ve_nav::{
    AgentParams, AgentStatus, Chase, Crowd, Flee, Goal, NavConfig, NavError, NavMesh, Path, Patrol,
    Wander, line_of_sight,
};

fn config(
    cell: f32,
    radius: f32,
    step: f32,
    slope: f32,
    min: [f32; 3],
    max: [f32; 3],
) -> NavConfig {
    NavConfig::new(cell, radius, 1.8, step, slope, min, max).expect("valid config")
}

fn room(cell: f32, radius: f32, step: f32) -> NavConfig {
    config(
        cell,
        radius,
        step,
        45.0,
        [0.0, -1.0, 0.0],
        [10.0, 3.0, 10.0],
    )
}

#[derive(Default)]
struct Soup {
    vertices: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

impl Soup {
    fn quad(&mut self, a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3]) {
        let base = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&[a, b, c, d]);
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn floor(&mut self, min: [f32; 2], max: [f32; 2], y: f32) {
        self.quad(
            [min[0], y, min[1]],
            [max[0], y, min[1]],
            [max[0], y, max[1]],
            [min[0], y, max[1]],
        );
    }

    fn wall(&mut self, x: f32, z0: f32, z1: f32, y0: f32, y1: f32) {
        self.quad([x, y0, z0], [x, y0, z1], [x, y1, z1], [x, y1, z0]);
    }

    /// A ramp quad from `(x0, y0)` to `(x1, y1)`, spanning all of `z`.
    fn ramp(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, z0: f32, z1: f32) {
        self.quad([x0, y0, z0], [x1, y1, z0], [x1, y1, z1], [x0, y0, z1]);
    }

    fn build(&self, config: &NavConfig) -> NavMesh {
        NavMesh::build(&self.vertices, &self.indices, config).expect("valid mesh")
    }
}

#[test]
fn flat_floor_is_fully_walkable() {
    let mut soup = Soup::default();
    soup.floor([0.0, 0.0], [10.0, 10.0], 0.0);
    let nav = soup.build(&room(0.5, 0.2, 0.4));
    let (nx, nz) = nav.dimensions();
    assert_eq!(nav.walkable_count(), nx * nz);
    assert!(nav.is_walkable([5.0, 0.0, 5.0]));
    assert_eq!(nav.height_at([5.0, 4.0, 5.0]), Some(0.0));
}

#[test]
fn a_wall_with_a_gap_routes_through_it() {
    let mut soup = Soup::default();
    soup.floor([0.0, 0.0], [10.0, 10.0], 0.0);
    soup.wall(5.0, 0.0, 3.5, 0.0, 2.5);
    soup.wall(5.0, 6.5, 10.0, 0.0, 2.5);
    let nav = soup.build(&config(
        0.1,
        0.0,
        0.4,
        45.0,
        [0.0, -1.0, 0.0],
        [10.0, 3.0, 10.0],
    ));
    let path = nav
        .find_path([1.0, 0.0, 1.0], [9.0, 0.0, 1.0])
        .expect("gap is wide enough");
    let expected = distance([1.0, 1.0], [5.0, 3.5]) + distance([5.0, 3.5], [9.0, 1.0]);
    assert!(
        (path.length() - expected).abs() <= 0.5,
        "path {} vs analytic {expected}",
        path.length()
    );
    assert!(
        path.points()
            .iter()
            .any(|p| p[0] > 4.0 && p[0] < 6.0 && p[2] > 3.0 && p[2] < 7.0),
        "the route should pass the gap: {:?}",
        path.points()
    );
}

#[test]
fn ramps_are_walkable_only_below_the_slope_limit() {
    let mut steep = Soup::default();
    steep.floor([0.0, 0.0], [2.0, 10.0], 0.0);
    steep.ramp(2.0, 0.0, 6.0, 4.0, 0.0, 10.0);
    let nav = steep.build(&config(
        0.25,
        0.1,
        0.4,
        30.0,
        [0.0, -1.0, 0.0],
        [6.0, 5.0, 10.0],
    ));
    assert!(
        !nav.is_walkable([4.0, 2.0, 5.0]),
        "a 45 degree ramp is too steep"
    );

    let mut shallow = Soup::default();
    shallow.floor([0.0, 0.0], [2.0, 10.0], 0.0);
    shallow.ramp(2.0, 0.0, 10.0, 4.0, 0.0, 10.0);
    let nav = shallow.build(&config(
        0.25,
        0.1,
        0.4,
        30.0,
        [0.0, -1.0, 0.0],
        [10.0, 5.0, 10.0],
    ));
    assert!(nav.is_walkable([6.0, 2.0, 5.0]), "a 27 degree ramp is fine");
    assert!(nav.height_at([6.0, 0.0, 5.0]).unwrap_or(-1.0) > 1.5);
}

#[test]
fn a_step_is_crossed_only_below_the_step_limit() {
    let mut high = Soup::default();
    high.floor([0.0, 0.0], [5.0, 10.0], 0.0);
    high.floor([5.0, 0.0], [10.0, 10.0], 1.0);
    high.wall(5.0, 0.0, 10.0, 0.0, 1.0);
    let nav = high.build(&room(0.25, 0.1, 0.4));
    assert!(matches!(
        nav.find_path([2.0, 0.0, 5.0], [8.0, 1.0, 5.0]),
        Err(NavError::NoPath { .. })
    ));

    let mut low = Soup::default();
    low.floor([0.0, 0.0], [5.0, 10.0], 0.0);
    low.floor([5.0, 0.0], [10.0, 10.0], 0.3);
    low.wall(5.0, 0.0, 10.0, 0.0, 0.3);
    let nav = low.build(&room(0.25, 0.1, 0.4));
    assert!(nav.find_path([2.0, 0.0, 5.0], [8.0, 0.3, 5.0]).is_ok());
    assert!(nav.is_walkable([8.0, 0.3, 5.0]));
}

#[test]
fn agent_radius_closes_a_narrow_gap() {
    let mut narrow = Soup::default();
    narrow.floor([0.0, 0.0], [10.0, 10.0], 0.0);
    narrow.wall(5.0, 0.0, 4.85, 0.0, 2.5);
    narrow.wall(5.0, 5.15, 10.0, 0.0, 2.5);
    let nav = narrow.build(&room(0.2, 0.3, 0.4));
    assert!(matches!(
        nav.find_path([1.0, 0.0, 5.0], [9.0, 0.0, 5.0]),
        Err(NavError::NoPath { .. })
    ));

    let mut wide = Soup::default();
    wide.floor([0.0, 0.0], [10.0, 10.0], 0.0);
    wide.wall(5.0, 0.0, 4.4, 0.0, 2.5);
    wide.wall(5.0, 5.6, 10.0, 0.0, 2.5);
    let nav = wide.build(&room(0.2, 0.3, 0.4));
    assert!(nav.find_path([1.0, 0.0, 5.0], [9.0, 0.0, 5.0]).is_ok());
}

#[test]
fn raycast_stops_at_a_wall() {
    let mut soup = Soup::default();
    soup.floor([0.0, 0.0], [10.0, 10.0], 0.0);
    soup.wall(5.0, 0.0, 5.0, 0.0, 2.5);
    let nav = soup.build(&room(0.25, 0.2, 0.4));
    let hit = nav
        .raycast([1.0, 1.0, 2.0], [9.0, 1.0, 2.0])
        .expect("the wall blocks this line");
    assert!((hit[0] - 5.0).abs() < 0.5, "hit at {hit:?}");
    assert!(!nav.line_of_sight([1.0, 1.0, 2.0], [9.0, 1.0, 2.0]));
    assert!(nav.line_of_sight([1.0, 1.0, 7.0], [9.0, 1.0, 7.0]));
}

#[test]
fn behaviours_choose_walkable_goals() {
    let mut soup = Soup::default();
    soup.floor([0.0, 0.0], [10.0, 10.0], 0.0);
    let nav = soup.build(&room(0.25, 0.2, 0.4));

    let mut wander = Wander::with_distance(9, 2.0);
    let goal = wander.goal(&nav, [5.0, 0.0, 5.0]).expect("a nearby cell");
    assert!(nav.is_walkable(goal));

    let mut patrol = Patrol::new(vec![[1.0, 0.0, 1.0], [9.0, 0.0, 9.0]]);
    let goal = patrol.goal(&nav, [1.0, 0.0, 1.0]).expect("first point");
    assert!(nav.is_walkable(goal));

    let mut chase = Chase::new([8.0, 0.0, 8.0]);
    let goal = chase.goal(&nav, [2.0, 0.0, 2.0]).expect("a nearby cell");
    assert!(nav.is_walkable(goal));

    let mut flee = Flee::new([5.0, 0.0, 5.0], 3.0);
    let goal = flee.goal(&nav, [5.0, 0.0, 4.0]).expect("a nearby cell");
    assert!(nav.is_walkable(goal));
    assert!(line_of_sight(&nav, [1.0, 0.0, 1.0], [9.0, 0.0, 9.0]));
}

#[test]
fn the_cell_budget_is_a_typed_error() {
    let mut soup = Soup::default();
    soup.floor([0.0, 0.0], [2000.0, 2000.0], 0.0);
    let config = config(1.0, 0.1, 0.4, 45.0, [0.0, -1.0, 0.0], [2000.0, 3.0, 2000.0]);
    assert!(matches!(
        NavMesh::build(&soup.vertices, &soup.indices, &config),
        Err(NavError::TooManyCells { .. })
    ));
}

#[test]
fn an_empty_mesh_is_a_typed_error() {
    let config = room(0.25, 0.2, 0.4);
    assert!(matches!(
        NavMesh::build(&[], &[], &config),
        Err(NavError::EmptyMesh)
    ));
    let vertices = [[0.0, 0.0, 0.0]];
    assert!(matches!(
        NavMesh::build(&vertices, &[], &config),
        Err(NavError::EmptyMesh)
    ));
}

#[test]
fn build_path_and_crowd_are_deterministic() {
    let make = || {
        let mut soup = Soup::default();
        soup.floor([0.0, 0.0], [20.0, 4.0], 0.0);
        soup.wall(10.0, 0.0, 1.0, 0.0, 2.5);
        soup.build(&config(
            0.25,
            0.3,
            0.4,
            45.0,
            [0.0, -1.0, 0.0],
            [20.0, 3.0, 4.0],
        ))
    };
    let a = make();
    let b = make();
    assert_eq!(state_hash(&a), state_hash(&b));
    let pa = a
        .find_path([1.0, 0.0, 2.0], [19.0, 0.0, 2.0])
        .expect("route");
    let pb = b
        .find_path([1.0, 0.0, 2.0], [19.0, 0.0, 2.0])
        .expect("route");
    assert_eq!(pa.points(), pb.points());

    assert_eq!(run_crowd(a), run_crowd(b));
}

#[test]
fn fifty_agents_cross_a_corridor_without_overlapping() {
    let mut soup = Soup::default();
    soup.floor([0.0, 0.0], [20.0, 6.0], 0.0);
    let nav = soup.build(&config(
        0.25,
        0.3,
        0.4,
        45.0,
        [0.0, -1.0, 0.0],
        [20.0, 3.0, 6.0],
    ));
    let mut crowd = Crowd::new(nav);
    let radius = 0.3;
    for k in 0..50 {
        let j = k % 25;
        let lane = 0.9 + (j % 5) as f32 * 1.05;
        let rank = (j / 5) as f32 * 0.9;
        // Spread the goals along the corridor instead of making everyone converge on one point,
        // so the test measures walking and passing rather than a deliberate queue at the exit.
        let (start, goal) = if k < 25 {
            ([1.0 + rank.min(4.0), 0.0, lane], [14.0 + rank, 0.0, lane])
        } else {
            ([19.0 - rank.min(4.0), 0.0, lane], [6.0 - rank, 0.0, lane])
        };
        crowd
            .add_agent(AgentParams::new(start, radius, 1.6, goal))
            .expect("agent fits");
    }
    let dt = 1.0 / 60.0;
    for _ in 0..6000 {
        crowd.step(dt);
        for (i, a) in crowd.agents().enumerate() {
            for b in crowd.agents().skip(i + 1) {
                let pa = a.position();
                let pb = b.position();
                let d = distance([pa[0], pa[2]], [pb[0], pb[2]]);
                let overlap = (a.radius() + b.radius() - d).max(0.0);
                assert!(
                    overlap <= radius * 0.1 + 1.0e-3,
                    "agents {i} overlap by {overlap}"
                );
            }
        }
    }
    let arrived = crowd
        .agents()
        .filter(|a| a.status() == AgentStatus::Arrived)
        .count();
    assert_eq!(arrived, 50, "everyone should reach the far side");
}

fn run_crowd(nav: NavMesh) -> u64 {
    let mut crowd = Crowd::new(nav);
    for k in 0..8 {
        let z = 0.7 + k as f32 * 0.3;
        crowd
            .add_agent(AgentParams::new([1.0, 0.0, z], 0.25, 1.2, [19.0, 0.0, z]))
            .expect("agent fits");
    }
    for _ in 0..300 {
        crowd.step(1.0 / 60.0);
    }
    let mut hasher = DefaultHasher::new();
    for agent in crowd.agents() {
        for value in agent.position() {
            value.to_bits().hash(&mut hasher);
        }
    }
    hasher.finish()
}

fn state_hash(nav: &NavMesh) -> u64 {
    let mut hasher = DefaultHasher::new();
    let (nx, nz) = nav.dimensions();
    nx.hash(&mut hasher);
    nz.hash(&mut hasher);
    let cell = nav.config().cell_size();
    let min = nav.config().bounds_min();
    for iz in 0..nz {
        for ix in 0..nx {
            let x = min[0] + (ix as f32 + 0.5) * cell;
            let z = min[2] + (iz as f32 + 0.5) * cell;
            let p = [x, 0.0, z];
            nav.is_walkable(p).hash(&mut hasher);
            nav.height_at(p).map(f32::to_bits).hash(&mut hasher);
        }
    }
    hasher.finish()
}

fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    let dx = a[0] - b[0];
    let dz = a[1] - b[1];
    (dx * dx + dz * dz).sqrt()
}

/// Keeps the `Path` import used in signatures above and checks a trivial path shape.
#[test]
fn a_one_cell_path_has_two_corners() {
    let mut soup = Soup::default();
    soup.floor([0.0, 0.0], [10.0, 10.0], 0.0);
    let nav = soup.build(&room(0.25, 0.2, 0.4));
    let path: Path = nav
        .find_path([5.0, 0.0, 5.0], [5.0, 0.0, 5.0])
        .expect("a point has a path");
    assert_eq!(path.len(), 2);
    assert!(path.length() < 1.0e-3);
}
