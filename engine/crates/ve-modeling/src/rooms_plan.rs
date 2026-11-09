//! `rooms.plan`: a building's walls and floors from a room list (`ve_world::rooms::layout`): a floor slab under every room and corridor, a
//! wall along every edge of every region, and an opening cut where each door is. The roof is left off so the inside can be seen and lit.

use ve_graph::{Node, Value as Param};
use ve_kernel::{ExecutionContext, KernelError, Solid};
use ve_world::rooms::{Axis, RoomError, RoomReq, RoomSpec, layout};

use crate::detail_panels::{cuboid, union_many};

/// Height of a door opening in metres.
const DOOR_HEIGHT: f64 = 2.1;

fn text<'a>(node: &'a Node, name: &str) -> &'a str {
    match node.params.get(name) {
        Some(Param::Str(text)) => text.as_str(),
        _ => "",
    }
}

fn number(node: &Node, name: &str) -> Result<f64, KernelError> {
    node.params.get(name).and_then(Param::as_f64).ok_or(KernelError::InvalidArgument("a number parameter is missing"))
}

/// `name:minW,minD,maxW,maxD; ...` as rooms (metres).
fn parse_rooms(source: &str) -> Result<Vec<RoomReq>, KernelError> {
    let bad = KernelError::InvalidArgument("rooms must be `name:minWidth,minDepth,maxWidth,maxDepth` separated by semicolons");
    let mut rooms = Vec::new();
    for item in source.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        let (name, sizes) = item.split_once(':').ok_or(bad.clone())?;
        let numbers: Result<Vec<f64>, _> = sizes.split(',').map(|n| n.trim().parse::<f64>()).collect();
        let numbers = numbers.map_err(|_| bad.clone())?;
        let [min_w, min_d, max_w, max_d] = numbers[..] else {
            return Err(bad);
        };
        rooms.push(RoomReq { name: name.trim().to_owned(), min: [min_w, min_d], max: [max_w, max_d] });
    }
    Ok(rooms)
}

/// `0-1; 1-2` as links between room numbers.
fn parse_links(source: &str) -> Result<Vec<(usize, usize)>, KernelError> {
    let bad = KernelError::InvalidArgument("links must be pairs of room numbers like `0-1; 1-2`");
    let mut links = Vec::new();
    for item in source.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        let (a, b) = item.split_once('-').ok_or(bad.clone())?;
        links.push((a.trim().parse().map_err(|_| bad.clone())?, b.trim().parse().map_err(|_| bad.clone())?));
    }
    Ok(links)
}

fn refusal(error: &RoomError) -> KernelError {
    KernelError::InvalidArgument(match error {
        RoomError::NoRooms => "the plan has no rooms",
        RoomError::TooManyRooms(_) => "a plan holds at most 64 rooms",
        RoomError::NoSuchRoom(..) => "a link names a room that does not exist",
        RoomError::SelfLink(_) => "a room is linked to itself",
        RoomError::BadSize(_) => "a room has a size range that is empty or not positive",
        RoomError::BadSetting(_) => "the cell, corridor or door width must be positive",
        RoomError::TooNarrowForDoor(_) => "a room is narrower than a door",
        RoomError::NotConnected(_) => "the links do not connect every room",
    })
}

/// The building for `node`.
pub(crate) fn build(node: &Node, context: &ExecutionContext) -> Result<Solid, KernelError> {
    let mut spec = RoomSpec::new(parse_rooms(text(node, "rooms"))?, parse_links(text(node, "links"))?, number(node, "seed")? as u64);
    spec.corridor_width = number(node, "corridor_width")?;
    spec.door_width = number(node, "door_width")?;
    let plan = layout(&spec).map_err(|e| refusal(&e))?;
    let (height, wall) = (number(node, "wall_height")?, number(node, "wall")?);
    let cell = plan.cell;
    let metres = |cells: i32| f64::from(cells) * cell;
    let mut parts = Vec::new();
    for region in &plan.regions {
        let r = region.rect;
        let (x0, x1, z0, z1) = (metres(r.x), metres(r.x + r.w), metres(r.z), metres(r.z + r.d));
        parts.push(cuboid([x0, -0.1, z0], [x1, 0.0, z1])?);
        let half = wall * 0.5;
        // the four walls, each centred on its edge and reaching half a wall past it so the corners are closed
        parts.push(cuboid([x0 - half, 0.0, z0 - half], [x1 + half, height, z0 + half])?);
        parts.push(cuboid([x0 - half, 0.0, z1 - half], [x1 + half, height, z1 + half])?);
        parts.push(cuboid([x0 - half, 0.0, z0 - half], [x0 + half, height, z1 + half])?);
        parts.push(cuboid([x1 - half, 0.0, z0 - half], [x1 + half, height, z1 + half])?);
    }
    let building = union_many(parts, context)?;
    let mut cutters = Vec::new();
    for door in &plan.doors {
        let (from, to, line) = (metres(door.from), metres(door.to), metres(door.line));
        let reach = wall * 1.5;
        cutters.push(match door.axis {
            Axis::X => cuboid([from, -0.05, line - reach], [to, DOOR_HEIGHT, line + reach])?,
            Axis::Z => cuboid([line - reach, -0.05, from], [line + reach, DOOR_HEIGHT, to])?,
        });
    }
    if cutters.is_empty() {
        return Ok(building);
    }
    building.difference_with_context(&union_many(cutters, context)?, context)
}
