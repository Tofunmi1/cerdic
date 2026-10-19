//! String pulling: the funnel algorithm over grid portals.

use glam::Vec2;

/// The two ends of the opening between two neighbouring cells, seen along the direction of travel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Portal {
    /// The end on the left of the direction of travel.
    pub(crate) left: Vec2,
    /// The end on the right of the direction of travel.
    pub(crate) right: Vec2,
}

/// Pulls the route `start -> end` tight through `portals`.
///
/// This is the simple stupid funnel algorithm. It returns the corners that matter, including the
/// two ends. Every returned segment stays inside the corridor of portals, so the result never
/// leaves the walkable cells and is never longer than the raw centre-to-centre grid route.
pub(crate) fn string_pull(start: Vec2, end: Vec2, portals: &[Portal]) -> Vec<Vec2> {
    let mut path = vec![start];
    let mut apex = start;
    let mut left = start;
    let mut right = start;
    let mut left_index = 0;
    let mut right_index = 0;

    let mut i = 0;
    while i < portals.len() {
        let portal = portals[i];
        let mut restarted = false;

        // Tighten the right side of the funnel.
        if triangle_area2(apex, right, portal.right) <= 0.0 {
            if same(apex, right) || triangle_area2(apex, left, portal.right) > 0.0 {
                right = portal.right;
                right_index = i;
            } else {
                path.push(left);
                apex = left;
                right = apex;
                right_index = left_index;
                i = left_index + 1;
                restarted = true;
            }
        }

        if !restarted {
            // Tighten the left side.
            if triangle_area2(apex, left, portal.left) >= 0.0 {
                if same(apex, left) || triangle_area2(apex, right, portal.left) < 0.0 {
                    left = portal.left;
                    left_index = i;
                } else {
                    path.push(right);
                    apex = right;
                    left = apex;
                    left_index = right_index;
                    i = right_index + 1;
                    restarted = true;
                }
            }
        }

        if !restarted {
            i += 1;
        }
    }

    path.push(end);
    path
}

/// Orders `p`/`q` so they follow the funnel algorithm's convention: `left` is the boundary on the
/// low side of the direction `from -> to`, `right` the high side.
pub(crate) fn orient(from: Vec2, to: Vec2, p: Vec2, q: Vec2) -> Portal {
    if triangle_area2(from, to, p) <= triangle_area2(from, to, q) {
        Portal { left: p, right: q }
    } else {
        Portal { left: q, right: p }
    }
}

/// Twice the signed area of `a, b, c`; positive when `c` is to the left of `a -> b`.
fn triangle_area2(a: Vec2, b: Vec2, c: Vec2) -> f32 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

fn same(a: Vec2, b: Vec2) -> bool {
    (a - b).length_squared() <= 1.0e-12
}
