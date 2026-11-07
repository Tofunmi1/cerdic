//! Floor plans: rooms joined to a corridor by doors, from a room list and the links between rooms, on a grid.
//!
//! The plan is a spine: one straight corridor with rooms on both sides, every room with a door to it. Every link `(a, b)` is then a short
//! walk (door, corridor, door) that passes through no third room, nothing overlaps, and corridors and doors are wide enough to walk. The
//! seed picks each room's size inside its range, the order rooms stand in along the corridor (a walk over the link graph, so linked rooms
//! end up close), which side of the corridor each takes and where its door sits. All decisions are integer cell arithmetic.
use std::collections::VecDeque;

/// Most rooms one plan holds.
pub const MAX_ROOMS: usize = 64;

/// A room to place: its width (x) and depth (z) range in metres.
#[derive(Clone, Debug, PartialEq)]
pub struct RoomReq {
    /// The room's name (for the caller; the plan refers to rooms by number).
    pub name: String,
    /// Smallest width and depth in metres.
    pub min: [f64; 2],
    /// Largest width and depth in metres.
    pub max: [f64; 2],
}

/// What to lay out.
#[derive(Clone, Debug, PartialEq)]
pub struct RoomSpec {
    /// The rooms.
    pub rooms: Vec<RoomReq>,
    /// Pairs of room numbers that must be a short walk apart.
    pub links: Vec<(usize, usize)>,
    /// The seed: the same spec gives the same plan.
    pub seed: u64,
    /// Grid cell in metres (default 0.5).
    pub cell: f64,
    /// Narrowest corridor in metres (default 1.6).
    pub corridor_width: f64,
    /// Narrowest door in metres (default 1.0).
    pub door_width: f64,
}

impl RoomSpec {
    /// A spec with the default cell, corridor width and door width.
    #[must_use]
    pub fn new(rooms: Vec<RoomReq>, links: Vec<(usize, usize)>, seed: u64) -> Self {
        Self { rooms, links, seed, cell: 0.5, corridor_width: 1.6, door_width: 1.0 }
    }
}

/// A rectangle in grid cells: `x` and `z` are the corner nearest the origin, `w` along x and `d` along z.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    /// Corner x in cells.
    pub x: i32,
    /// Corner z in cells.
    pub z: i32,
    /// Width (x) in cells.
    pub w: i32,
    /// Depth (z) in cells.
    pub d: i32,
}

/// What a region is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Room number `n` of the spec.
    Room(usize),
    /// A corridor.
    Corridor,
}

/// One rectangle of the plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    /// Room or corridor.
    pub kind: Kind,
    /// Where it is.
    pub rect: Rect,
}

/// The direction a door runs in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// Along x, on the grid line z = `line`.
    X,
    /// Along z, on the grid line x = `line`.
    Z,
}

/// An opening in the wall two regions share.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Door {
    /// Index into [`Layout::regions`].
    pub a: usize,
    /// Index into [`Layout::regions`].
    pub b: usize,
    /// The direction the door runs in.
    pub axis: Axis,
    /// The grid line it sits on.
    pub line: i32,
    /// Where it starts along the wall, in cells.
    pub from: i32,
    /// Where it ends along the wall, in cells.
    pub to: i32,
}

/// A finished plan.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// The rooms and corridors.
    pub regions: Vec<Region>,
    /// The doors between them.
    pub doors: Vec<Door>,
    /// Grid cell in metres.
    pub cell: f64,
}

/// Why a spec was refused.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RoomError {
    /// There are no rooms.
    #[error("there are no rooms")]
    NoRooms,
    /// More than [`MAX_ROOMS`] rooms.
    #[error("{0} rooms; a plan holds at most {MAX_ROOMS}")]
    TooManyRooms(usize),
    /// A link names a room that does not exist.
    #[error("link ({0}, {1}) names a room that does not exist")]
    NoSuchRoom(usize, usize),
    /// A room is linked to itself.
    #[error("room {0} is linked to itself")]
    SelfLink(usize),
    /// A size range is empty, not finite or not positive.
    #[error("room {0} has a size range that is empty, not finite or not positive")]
    BadSize(usize),
    /// A grid or width setting is not a positive finite number.
    #[error("{0} must be a positive, finite number")]
    BadSetting(&'static str),
    /// A room is narrower than a door.
    #[error("room {0} is narrower than a door; make it wider or the door narrower")]
    TooNarrowForDoor(usize),
    /// The links do not join every room into one group.
    #[error("the links do not connect every room: room {0} cannot be reached from room 0")]
    NotConnected(usize),
}

/// Seeded generator (splitmix64): the same seed, the same numbers, on every platform.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number in `0..=high`.
    fn upto(&mut self, high: u32) -> u32 {
        u32::try_from(self.next() % (u64::from(high) + 1)).unwrap_or(0)
    }
}

fn cells_up(metres: f64, cell: f64) -> i32 {
    #[expect(clippy::cast_possible_truncation, reason = "sizes are checked small before this is called")]
    let n = (metres / cell - 1e-9).ceil() as i32;
    n.max(1)
}

fn cells_down(metres: f64, cell: f64) -> i32 {
    #[expect(clippy::cast_possible_truncation, reason = "sizes are checked small before this is called")]
    let n = (metres / cell + 1e-9).floor() as i32;
    n.max(1)
}

fn positive(x: f64) -> bool {
    x.is_finite() && x > 0.0
}

fn validate(spec: &RoomSpec) -> Result<(), RoomError> {
    let n = spec.rooms.len();
    if n == 0 {
        return Err(RoomError::NoRooms);
    }
    if n > MAX_ROOMS {
        return Err(RoomError::TooManyRooms(n));
    }
    for (name, value) in [("cell", spec.cell), ("corridor_width", spec.corridor_width), ("door_width", spec.door_width)] {
        if !positive(value) {
            return Err(RoomError::BadSetting(name));
        }
    }
    for (i, room) in spec.rooms.iter().enumerate() {
        let sizes = room.min.iter().chain(&room.max);
        if sizes.clone().any(|s| !positive(*s) || *s > 1000.0 * spec.cell.max(1.0)) || room.min[0] > room.max[0] || room.min[1] > room.max[1] {
            return Err(RoomError::BadSize(i));
        }
        if cells_down(room.max[0], spec.cell).min(cells_up(room.min[0], spec.cell)) < cells_up(spec.door_width, spec.cell) {
            return Err(RoomError::TooNarrowForDoor(i));
        }
    }
    for &(a, b) in &spec.links {
        if a >= n || b >= n {
            return Err(RoomError::NoSuchRoom(a, b));
        }
        if a == b {
            return Err(RoomError::SelfLink(a));
        }
    }
    Ok(())
}

fn adjacency(spec: &RoomSpec) -> Vec<Vec<usize>> {
    let mut edges = vec![Vec::new(); spec.rooms.len()];
    for &(a, b) in &spec.links {
        edges[a].push(b);
        edges[b].push(a);
    }
    edges
}

fn first_unreached(edges: &[Vec<usize>]) -> Option<usize> {
    let mut seen = vec![false; edges.len()];
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
    seen.iter().position(|s| !*s)
}

/// The order rooms stand in: a depth-first walk over the links from a random room, neighbours in random order, so linked rooms are close.
fn walk_order(edges: &[Vec<usize>], rng: &mut Rng) -> Vec<usize> {
    let n = edges.len();
    let start = rng.upto(u32::try_from(n - 1).unwrap_or(0)) as usize;
    let mut seen = vec![false; n];
    let mut order = Vec::with_capacity(n);
    let mut stack = vec![start];
    while let Some(at) = stack.pop() {
        if seen[at] {
            continue;
        }
        seen[at] = true;
        order.push(at);
        let mut next: Vec<usize> = edges[at].iter().copied().filter(|j| !seen[*j]).collect();
        for k in (1..next.len()).rev() {
            let swap = rng.upto(u32::try_from(k).unwrap_or(0)) as usize;
            next.swap(k, swap);
        }
        stack.extend(next);
    }
    order
}

/// Lays the rooms out.
///
/// # Errors
/// A [`RoomError`] naming what is wrong with the spec: no rooms, too many, a bad link, a bad size or setting, a room narrower than a door, or
/// links that do not connect every room.
pub fn layout(spec: &RoomSpec) -> Result<Layout, RoomError> {
    validate(spec)?;
    let edges = adjacency(spec);
    if let Some(room) = first_unreached(&edges) {
        return Err(RoomError::NotConnected(room));
    }
    let mut rng = Rng(spec.seed ^ 0xA076_1D64_78BD_642F);
    let cell = spec.cell;
    let sized: Vec<(i32, i32)> = spec
        .rooms
        .iter()
        .map(|r| {
            let size = |axis: usize, rng: &mut Rng| {
                let low = cells_up(r.min[axis], cell);
                let high = cells_down(r.max[axis], cell).max(low);
                low + i32::try_from(rng.upto(u32::try_from(high - low).unwrap_or(0))).unwrap_or(0)
            };
            let w = size(0, &mut rng);
            (w, size(1, &mut rng))
        })
        .collect();
    if spec.rooms.len() == 1 {
        let (w, d) = sized[0];
        return Ok(Layout { regions: vec![Region { kind: Kind::Room(0), rect: Rect { x: 0, z: 0, w, d } }], doors: Vec::new(), cell });
    }
    // Rooms go on the side of the corridor that is shorter so far.
    let order = walk_order(&edges, &mut rng);
    let (mut north, mut south): (Vec<usize>, Vec<usize>) = (Vec::new(), Vec::new());
    let (mut north_len, mut south_len) = (0, 0);
    for room in order {
        let to_north = north_len < south_len || (north_len == south_len && rng.upto(1) == 0);
        if to_north {
            north_len += sized[room].0;
            north.push(room);
        } else {
            south_len += sized[room].0;
            south.push(room);
        }
    }
    let wide = cells_up(spec.corridor_width, cell);
    let door = cells_up(spec.door_width, cell);
    let north_depth = north.iter().map(|&r| sized[r].1).max().unwrap_or(0);
    let length = north_len.max(south_len).max(wide);
    let corridor = Rect { x: 0, z: north_depth, w: length, d: wide };
    let mut regions = vec![Region { kind: Kind::Corridor, rect: corridor }];
    let mut doors = Vec::new();
    for (side, rooms) in [(true, &north), (false, &south)] {
        let mut x = 0;
        for &room in rooms {
            let (w, d) = sized[room];
            let z = if side { north_depth - d } else { north_depth + wide };
            let rect = Rect { x, z, w, d };
            let at = u32::try_from(w - door).unwrap_or(0);
            let from = x + i32::try_from(rng.upto(at)).unwrap_or(0);
            let line = if side { north_depth } else { north_depth + wide };
            doors.push(Door { a: regions.len(), b: 0, axis: Axis::X, line, from, to: from + door });
            regions.push(Region { kind: Kind::Room(room), rect });
            x += w;
        }
    }
    Ok(Layout { regions, doors, cell })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room(min: f64, max: f64) -> RoomReq {
        RoomReq { name: "r".to_owned(), min: [min, min], max: [max, max] }
    }

    #[test]
    fn the_same_seed_gives_the_same_plan_and_every_room_has_a_door_to_the_corridor() {
        let spec = RoomSpec::new((0..6).map(|_| room(3.0, 5.0)).collect(), (0..5).map(|i| (i, i + 1)).collect(), 4);
        let plan = layout(&spec).unwrap();
        assert_eq!(plan, layout(&spec).unwrap());
        assert_eq!(plan.regions.len(), 7);
        assert_eq!(plan.doors.len(), 6);
        assert!(plan.doors.iter().all(|d| d.b == 0));
    }

    #[test]
    fn unconnected_links_name_the_room_that_cannot_be_reached() {
        let spec = RoomSpec::new(vec![room(3.0, 4.0), room(3.0, 4.0), room(3.0, 4.0)], vec![(0, 1)], 1);
        assert_eq!(layout(&spec), Err(RoomError::NotConnected(2)));
    }

    #[test]
    fn a_room_narrower_than_a_door_is_refused() {
        let spec = RoomSpec::new(vec![room(0.5, 0.8), room(3.0, 4.0)], vec![(0, 1)], 1);
        assert_eq!(layout(&spec), Err(RoomError::TooNarrowForDoor(0)));
    }
}
