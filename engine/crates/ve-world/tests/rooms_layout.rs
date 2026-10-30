//! `rooms::layout`: rooms and corridors from a room list and the links between rooms, on a grid, with doors. The floor plan of a ship, a
//! station or a bunker; every room reachable, every link a short walk, nothing overlapping, the same seed the same plan.
use std::collections::VecDeque;
use ve_world::rooms::{Axis, Door, Kind, Layout, Rect, RoomReq, RoomSpec, layout};

fn room(name: &str, min: [f64; 2], max: [f64; 2]) -> RoomReq {
    RoomReq {
        name: name.to_owned(),
        min,
        max,
    }
}

fn station(seed: u64) -> RoomSpec {
    RoomSpec::new(
        vec![
            room("airlock", [3.0, 3.0], [4.0, 4.0]),
            room("hub", [6.0, 6.0], [8.0, 8.0]),
            room("bridge", [6.0, 4.0], [9.0, 6.0]),
            room("engine", [8.0, 6.0], [12.0, 9.0]),
            room("quarters", [5.0, 5.0], [7.0, 7.0]),
            room("lab", [5.0, 4.0], [8.0, 6.0]),
        ],
        vec![(0, 1), (1, 2), (1, 3), (1, 4), (4, 5)],
        seed,
    )
}

fn overlap(a: &Rect, b: &Rect) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w && a.z < b.z + b.d && b.z < a.z + a.d
}

/// The shared wall of two touching rectangles: its direction, its grid line and the stretch along it they both cover.
fn shared_wall(a: &Rect, b: &Rect) -> Option<(Axis, i32, i32, i32)> {
    let span = |a0: i32, a1: i32, b0: i32, b1: i32| {
        let (lo, hi) = (a0.max(b0), a1.min(b1));
        (hi > lo).then_some((lo, hi))
    };
    if a.z + a.d == b.z || b.z + b.d == a.z {
        let line = if a.z + a.d == b.z { b.z } else { a.z };
        return span(a.x, a.x + a.w, b.x, b.x + b.w).map(|(lo, hi)| (Axis::X, line, lo, hi));
    }
    if a.x + a.w == b.x || b.x + b.w == a.x {
        let line = if a.x + a.w == b.x { b.x } else { a.x };
        return span(a.z, a.z + a.d, b.z, b.z + b.d).map(|(lo, hi)| (Axis::Z, line, lo, hi));
    }
    None
}

fn door_graph(plan: &Layout) -> Vec<Vec<usize>> {
    let mut edges = vec![Vec::new(); plan.regions.len()];
    for Door { a, b, .. } in &plan.doors {
        edges[*a].push(*b);
        edges[*b].push(*a);
    }
    edges
}

fn room_region(plan: &Layout, room: usize) -> usize {
    plan.regions
        .iter()
        .position(|r| r.kind == Kind::Room(room))
        .unwrap_or_else(|| panic!("room {room} has no region"))
}

/// Whether room `from` reaches room `to` through doors, passing only through corridors in between.
fn corridor_walk(plan: &Layout, from: usize, to: usize) -> bool {
    let edges = door_graph(plan);
    let (start, goal) = (room_region(plan, from), room_region(plan, to));
    let mut seen = vec![false; plan.regions.len()];
    let mut queue = VecDeque::from([start]);
    seen[start] = true;
    while let Some(at) = queue.pop_front() {
        if at == goal {
            return true;
        }
        for &next in &edges[at] {
            let passable = next == goal || plan.regions[next].kind == Kind::Corridor;
            if passable && !seen[next] {
                seen[next] = true;
                queue.push_back(next);
            }
        }
    }
    false
}

fn check_all(spec: &RoomSpec, plan: &Layout) {
    let cell = spec.cell;
    // Every room appears once, within its size range (rounded up to whole cells).
    for (index, req) in spec.rooms.iter().enumerate() {
        let found: Vec<_> = plan
            .regions
            .iter()
            .filter(|r| r.kind == Kind::Room(index))
            .collect();
        assert_eq!(found.len(), 1, "{}: one region", req.name);
        let rect = found[0].rect;
        let (w, d) = (f64::from(rect.w) * cell, f64::from(rect.d) * cell);
        assert!(
            w >= req.min[0] - 1e-9 && w <= req.max[0] + cell + 1e-9,
            "{}: width {w}",
            req.name
        );
        assert!(
            d >= req.min[1] - 1e-9 && d <= req.max[1] + cell + 1e-9,
            "{}: depth {d}",
            req.name
        );
    }
    // Nothing overlaps; corridors are wide enough.
    for (i, a) in plan.regions.iter().enumerate() {
        assert!(a.rect.w > 0 && a.rect.d > 0, "region {i} is empty");
        for b in &plan.regions[i + 1..] {
            assert!(
                !overlap(&a.rect, &b.rect),
                "{:?} overlaps {:?}",
                a.rect,
                b.rect
            );
        }
        if a.kind == Kind::Corridor {
            let narrow = f64::from(a.rect.w.min(a.rect.d)) * cell;
            assert!(
                narrow >= spec.corridor_width - 1e-9,
                "corridor {narrow} m wide"
            );
        }
    }
    // Every door sits in the wall two regions share, and is wide enough.
    for door in &plan.doors {
        let (a, b) = (&plan.regions[door.a].rect, &plan.regions[door.b].rect);
        let (axis, line, lo, hi) = shared_wall(a, b)
            .unwrap_or_else(|| panic!("door between {a:?} and {b:?}, which do not touch"));
        assert_eq!(door.axis, axis);
        assert_eq!(door.line, line);
        assert!(
            door.from >= lo && door.to <= hi && door.to > door.from,
            "door {}..{} outside wall {lo}..{hi}",
            door.from,
            door.to
        );
        assert!(
            f64::from(door.to - door.from) * cell >= spec.door_width - 1e-9,
            "door too narrow"
        );
    }
    // Everything is connected, and every link is a short walk (a door, or corridors only).
    let edges = door_graph(plan);
    let mut seen = vec![false; plan.regions.len()];
    let mut queue = VecDeque::from([0_usize]);
    seen[0] = true;
    while let Some(at) = queue.pop_front() {
        for &next in &edges[at] {
            if !seen[next] {
                seen[next] = true;
                queue.push_back(next);
            }
        }
    }
    assert!(seen.iter().all(|s| *s), "some region cannot be reached");
    for &(a, b) in &spec.links {
        assert!(
            corridor_walk(plan, a, b),
            "rooms {a} and {b} are linked but not connected by corridors only"
        );
    }
    // The plan starts at the origin and stays compact.
    assert_eq!(plan.regions.iter().map(|r| r.rect.x).min(), Some(0));
    assert_eq!(plan.regions.iter().map(|r| r.rect.z).min(), Some(0));
    let (mut max_x, mut max_z) = (0, 0);
    for r in &plan.regions {
        max_x = max_x.max(r.rect.x + r.rect.w);
        max_z = max_z.max(r.rect.z + r.rect.d);
    }
    let hull = f64::from(max_x) * f64::from(max_z) * cell * cell;
    let rooms: f64 = spec.rooms.iter().map(|r| r.max[0] * r.max[1]).sum();
    assert!(
        hull <= 8.0 * rooms,
        "bounding area {hull} m2 for {rooms} m2 of rooms"
    );
}

#[test]
fn a_station_plan_satisfies_every_rule() {
    let spec = station(1);
    check_all(&spec, &layout(&spec).unwrap());
}

#[test]
fn fifty_seeds_all_satisfy_every_rule() {
    for seed in 0..50 {
        let spec = station(seed);
        check_all(
            &spec,
            &layout(&spec).unwrap_or_else(|e| panic!("seed {seed}: {e}")),
        );
    }
}

#[test]
fn the_same_seed_is_the_same_plan_and_other_seeds_differ() {
    assert_eq!(layout(&station(7)).unwrap(), layout(&station(7)).unwrap());
    let plans: Vec<_> = (0..8).map(|s| layout(&station(s)).unwrap()).collect();
    assert!(
        plans.iter().any(|p| *p != plans[0]),
        "eight seeds, one plan"
    );
}

#[test]
fn fixed_size_rooms_are_exactly_that_size() {
    let spec = RoomSpec::new(
        vec![
            room("a", [4.0, 4.0], [4.0, 4.0]),
            room("b", [3.0, 5.0], [3.0, 5.0]),
        ],
        vec![(0, 1)],
        3,
    );
    let plan = layout(&spec).unwrap();
    check_all(&spec, &plan);
    let a = plan.regions[room_region(&plan, 0)].rect;
    assert_eq!(
        (f64::from(a.w) * spec.cell, f64::from(a.d) * spec.cell),
        (4.0, 4.0)
    );
}

#[test]
fn a_long_chain_of_rooms_fits_and_connects() {
    let rooms: Vec<_> = (0..24)
        .map(|i| room(&format!("r{i}"), [3.0, 3.0], [5.0, 5.0]))
        .collect();
    let links: Vec<_> = (0..23).map(|i| (i, i + 1)).collect();
    for seed in 0..10 {
        let spec = RoomSpec::new(rooms.clone(), links.clone(), seed);
        check_all(&spec, &layout(&spec).unwrap());
    }
}

#[test]
fn a_ring_and_a_branching_tree_both_work() {
    let rooms: Vec<_> = (0..6)
        .map(|i| room(&format!("r{i}"), [4.0, 4.0], [6.0, 6.0]))
        .collect();
    let ring: Vec<_> = (0..6).map(|i| (i, (i + 1) % 6)).collect();
    let spec = RoomSpec::new(rooms.clone(), ring, 5);
    check_all(&spec, &layout(&spec).unwrap());
    let tree = vec![(0, 1), (0, 2), (1, 3), (1, 4), (2, 5)];
    let spec = RoomSpec::new(rooms, tree, 5);
    check_all(&spec, &layout(&spec).unwrap());
}

#[test]
fn bad_input_is_refused_with_an_error_not_a_panic() {
    let ok = station(1);
    let mut spec = ok.clone();
    spec.links.push((0, 99));
    assert!(
        layout(&spec).is_err(),
        "a link to a room that does not exist"
    );
    let mut spec = ok.clone();
    spec.links.push((2, 2));
    assert!(layout(&spec).is_err(), "a room linked to itself");
    let mut spec = ok.clone();
    spec.rooms[0].min = [9.0, 9.0];
    assert!(layout(&spec).is_err(), "min larger than max");
    let mut spec = ok.clone();
    spec.cell = 0.0;
    assert!(layout(&spec).is_err(), "a zero cell");
    let mut spec = ok.clone();
    spec.links.retain(|l| *l != (4, 5));
    assert!(
        layout(&spec).is_err(),
        "a room no link reaches cannot be connected"
    );
    assert!(
        layout(&RoomSpec::new(Vec::new(), Vec::new(), 1)).is_err(),
        "no rooms"
    );
    let many: Vec<_> = (0..65)
        .map(|i| room(&format!("r{i}"), [3.0, 3.0], [4.0, 4.0]))
        .collect();
    let chain: Vec<_> = (0..64).map(|i| (i, i + 1)).collect();
    assert!(
        layout(&RoomSpec::new(many, chain, 1)).is_err(),
        "more than 64 rooms"
    );
}

#[test]
fn one_room_is_a_plan_with_no_doors() {
    let spec = RoomSpec::new(vec![room("cell", [3.0, 3.0], [4.0, 4.0])], Vec::new(), 1);
    let plan = layout(&spec).unwrap();
    assert_eq!(plan.regions.len(), 1);
    assert!(plan.doors.is_empty());
}
