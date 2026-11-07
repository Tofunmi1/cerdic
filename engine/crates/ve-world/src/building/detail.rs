//! Street detail for the district building generator, all behind `detail` (0 to 1; 0 adds nothing): shop awnings, a stoop at the door of a low house, fire
//! escapes on the street front of a mid-rise, a cornice on the roof edge, a wooden water tower on a roof and a spire on a tall tower. Every piece is placed
//! from the street-facing edge of the footprint, in that edge's own frame, so it works for any lot outline.
use super::{BAY, Building, BuildingParams, Mesh, Rng, area, inset};
use std::f32::consts::TAU;

/// The street-facing edge of a footprint: a corner, the direction along it, the outward normal and the length.
struct Front {
    a: [f32; 2],
    t: [f32; 2],
    n: [f32; 2],
    length: f32,
}

impl Front {
    fn new(outline: &[[f32; 2]], front: usize) -> Option<Self> {
        let a = *outline.get(front)?;
        let b = *outline.get((front + 1) % outline.len())?;
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let length = (dx * dx + dz * dz).sqrt();
        (length > 1e-3).then(|| Self {
            a,
            t: [dx / length, dz / length],
            n: [dz / length, -dx / length],
            length,
        })
    }

    /// The point `s` metres along the front and `d` metres out from it.
    fn at(&self, s: f32, d: f32) -> [f32; 2] {
        [
            self.a[0] + self.t[0] * s + self.n[0] * d,
            self.a[1] + self.t[1] * s + self.n[1] * d,
        ]
    }

    /// The turn that lines a box's long side up with the front.
    fn yaw(&self) -> f32 {
        self.t[1].atan2(self.t[0])
    }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn unit(a: [f32; 3]) -> [f32; 3] {
    let l = dot(a, a).sqrt().max(1e-9);
    [a[0] / l, a[1] / l, a[2] / l]
}

fn lerp(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// A box that runs from `p0` to `p1` (a sloping awning, a stair), `width` across along the horizontal `side` and `thick` through, every face wound outward.
/// `side` must be at right angles to the run.
fn beam(mesh: &mut Mesh, p0: [f32; 3], p1: [f32; 3], side: [f32; 2], width: f32, thick: f32) {
    let run = unit(sub(p1, p0));
    let side = [side[0], 0.0, side[1]];
    let up = unit(cross(side, run));
    let corner = |end: [f32; 3], sx: f32, su: f32| {
        [
            end[0] + side[0] * sx * width * 0.5 + up[0] * su * thick * 0.5,
            end[1] + side[1] * sx * width * 0.5 + up[1] * su * thick * 0.5,
            end[2] + side[2] * sx * width * 0.5 + up[2] * su * thick * 0.5,
        ]
    };
    let ends = [p0, p1];
    let centre = lerp(p0, p1, 0.5);
    let mut quad = |q: [[f32; 3]; 4]| {
        let face = lerp(q[0], q[2], 0.5);
        let normal = cross(sub(q[1], q[0]), sub(q[3], q[0]));
        let q = if dot(normal, sub(face, centre)) < 0.0 {
            [q[0], q[3], q[2], q[1]]
        } else {
            q
        };
        let uv = |p: [f32; 3]| [(p[0] + p[2]) / BAY, p[1] / BAY];
        mesh.triangle([(q[0], uv(q[0])), (q[1], uv(q[1])), (q[2], uv(q[2]))]);
        mesh.triangle([(q[0], uv(q[0])), (q[2], uv(q[2])), (q[3], uv(q[3]))]);
    };
    for sx in [-1.0, 1.0] {
        quad([
            corner(ends[0], sx, -1.0),
            corner(ends[1], sx, -1.0),
            corner(ends[1], sx, 1.0),
            corner(ends[0], sx, 1.0),
        ]);
    }
    for su in [-1.0, 1.0] {
        quad([
            corner(ends[0], -1.0, su),
            corner(ends[1], -1.0, su),
            corner(ends[1], 1.0, su),
            corner(ends[0], 1.0, su),
        ]);
    }
    for end in ends {
        quad([
            corner(end, -1.0, -1.0),
            corner(end, 1.0, -1.0),
            corner(end, 1.0, 1.0),
            corner(end, -1.0, 1.0),
        ]);
    }
}

fn at3(p: [f32; 2], y: f32) -> [f32; 3] {
    [p[0], y, p[1]]
}

/// A sloping awning over every five metres of the shop front, with a short valance at its outer edge.
fn awnings(mesh: &mut Mesh, front: &Front, shop: f32) {
    let (top, drop, out) = (shop - 0.65, 0.8, 1.8);
    if top < 2.6 {
        return;
    }
    #[expect(clippy::cast_possible_truncation, reason = "a few bays")]
    #[expect(clippy::cast_sign_loss, reason = "the length is positive")]
    let bays = ((front.length / 5.0).floor() as usize).max(1);
    let width = front.length / bays as f32;
    for i in 0..bays {
        let s = (i as f32 + 0.5) * width;
        beam(
            mesh,
            at3(front.at(s, 0.0), top),
            at3(front.at(s, out), top - drop),
            front.t,
            width * 0.8,
            0.08,
        );
        mesh.block(
            front.at(s, out),
            [width * 0.8, 0.3, 0.06],
            front.yaw(),
            top - drop - 0.3,
        );
    }
}

/// Four stone steps up to the door of a low house.
fn stoop(shop: &mut Mesh, front: &Front) {
    let s = front.length * 0.3;
    for i in 0..4_u8 {
        let (h, depth) = (0.35 * f32::from(i + 1), 0.55 * f32::from(4 - i));
        shop.block(front.at(s, depth * 0.5), [1.6, h, depth], front.yaw(), 0.0);
    }
}

/// An iron fire escape up the street front: a platform with a rail at every floor and a stair between them that zigzags.
fn fire_escape(metal: &mut Mesh, front: &Front, params: &BuildingParams, shop: f32, rng: &mut Rng) {
    let width = (front.length * 0.3).clamp(2.2, 3.2);
    let s0 = front.length * (0.3 + 0.4 * rng.next());
    let yaw = front.yaw();
    let mut levels = Vec::new();
    let mut y = shop + params.floor;
    while y <= params.height - params.floor * 0.6 {
        levels.push(y);
        y += params.floor;
    }
    for (k, &y) in levels.iter().enumerate() {
        metal.block(front.at(s0, 0.55), [width, 0.08, 1.1], yaw, y);
        metal.block(front.at(s0, 1.08), [width, 0.05, 0.05], yaw, y + 1.0);
        for side in [-1.0, 1.0] {
            let x = s0 + side * (width * 0.5 - 0.03);
            metal.block(front.at(x, 0.55), [0.05, 0.05, 1.1], yaw, y + 1.0);
            metal.block(
                front.at(s0 + side * width * 0.5, 1.07),
                [0.05, 1.0, 0.05],
                yaw,
                y + 0.08,
            );
        }
        if let Some(&next) = levels.get(k + 1) {
            let (from, to) = if k % 2 == 0 { (-1.0, 1.0) } else { (1.0, -1.0) };
            let a = at3(front.at(s0 + from * (width * 0.5 - 0.4), 0.9), y + 0.08);
            let b = at3(front.at(s0 + to * (width * 0.5 - 0.4), 0.9), next);
            beam(metal, a, b, front.n, 0.5, 0.06);
        }
    }
}

/// A band that projects from the top of the wall, with its flat top in the roof.
fn cornice(building: &mut Building, outline: &[[f32; 2]], y: f32, floor: f32) {
    let outer = inset(outline, -0.35);
    building.walls.walls(&outer, y - 0.5, y, floor);
    building.roof.ring(&outer, outline, y);
}

/// A wooden tank on four legs with a cap, the water tower of a New York roof.
fn water_tower(roof: &mut Mesh, outline: &[[f32; 2]], y: f32, rng: &mut Rng) {
    let n = outline.len() as f32;
    let c = outline
        .iter()
        .fold([0.0, 0.0], |s, p| [s[0] + p[0] / n, s[1] + p[1] / n]);
    let at = [
        c[0] + (rng.next() - 0.5) * 2.0,
        c[1] + (rng.next() - 0.5) * 2.0,
    ];
    for k in 0..4_u8 {
        let a = TAU * (f32::from(k) + 0.5) / 4.0;
        roof.block(
            [at[0] + a.cos() * 0.8, at[1] + a.sin() * 0.8],
            [0.14, 2.6, 0.14],
            0.0,
            y,
        );
    }
    roof.tank(at, 1.2, 2.6, y + 2.6);
    roof.tank(at, 0.5, 0.7, y + 5.2);
}

/// The stepped crown and the mast of a tall tower, standing on its topmost roof.
fn spire(roof: &mut Mesh, outline: &[[f32; 2]], y: f32) {
    let n = outline.len() as f32;
    let c = outline
        .iter()
        .fold([0.0, 0.0], |s, p| [s[0] + p[0] / n, s[1] + p[1] / n]);
    let reach = outline.iter().fold(f32::MAX, |m, p| {
        m.min((p[0] - c[0]).abs().max((p[1] - c[1]).abs()))
    });
    let w = (reach * 1.1).clamp(1.0, 9.0);
    roof.block(c, [w, 5.0, w], 0.0, y);
    roof.block(c, [w * 0.6, 7.0, w * 0.6], 0.0, y + 5.0);
    roof.block(c, [0.5, 16.0, 0.5], 0.0, y + 12.0);
}

/// Adds the street detail the `detail` amount asks for to a finished building.
pub(super) fn add(
    building: &mut Building,
    params: &BuildingParams,
    shop: f32,
    roof_outline: &[[f32; 2]],
    rng: &mut Rng,
) {
    let detail = params.detail;
    if detail <= 0.0 {
        return;
    }
    let tower = params.height >= 90.0;
    let front = Front::new(&params.footprint, params.front);
    if let Some(front) = &front {
        if detail >= 0.2 {
            awnings(&mut building.awning, front, shop);
        }
        if detail >= 0.4 && params.height < 16.0 && front.length >= 5.0 {
            stoop(&mut building.shop, front);
        }
        let chance = ((detail - 0.3) / 0.7).clamp(0.0, 1.0);
        if (18.0..32.0).contains(&params.height) && front.length >= 8.0 && rng.next() < chance {
            fire_escape(&mut building.metal, front, params, shop, rng);
        }
    }
    if detail >= 0.5 && params.height < 50.0 {
        cornice(building, &params.footprint, params.height, params.floor);
    }
    if detail >= 0.5
        && (18.0..90.0).contains(&params.height)
        && area(roof_outline) >= 100.0
        && rng.next() < 0.5
    {
        water_tower(&mut building.roof, roof_outline, params.height, rng);
    }
    if detail >= 0.3 && tower {
        spire(&mut building.roof, roof_outline, params.height);
    }
}
