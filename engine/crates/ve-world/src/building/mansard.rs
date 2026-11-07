//! A mansard roof for a mid-rise building: a steep slope one storey high drawn in from the eaves, a flat top, and dormer windows standing on the slope.
use super::{BAY, Mesh};

/// The roof of a building.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Roof {
    /// A flat roof with its clutter (what a building was before roofs could be chosen).
    #[default]
    Flat,
    /// A steep mansard slope one storey high with dormers; only buildings from three storeys up to 50 m take it, others stay flat.
    Mansard,
}

/// Whether a building of `height` with storeys of `floor` metres takes a mansard roof.
pub(super) fn fits(height: f32, floor: f32) -> bool {
    height < 50.0 && height >= floor * 3.0 + floor * 1.35 + floor
}

impl Mesh {
    /// A sloped ring up and in: from `outer` at height `y0` to `inner` at `y1`, facing up and out.
    fn slope(&mut self, outer: &[[f32; 2]], y0: f32, inner: &[[f32; 2]], y1: f32) {
        let uv = |p: [f32; 2]| [p[0] / BAY, p[1] / BAY];
        let n = outer.len();
        for i in 0..n {
            let j = (i + 1) % n;
            let low = |q: [f32; 2]| [q[0], y0, q[1]];
            let high = |q: [f32; 2]| [q[0], y1, q[1]];
            self.triangle([
                (low(outer[i]), uv(outer[i])),
                (high(inner[j]), uv(inner[j])),
                (low(outer[j]), uv(outer[j])),
            ]);
            self.triangle([
                (low(outer[i]), uv(outer[i])),
                (high(inner[i]), uv(inner[i])),
                (high(inner[j]), uv(inner[j])),
            ]);
        }
    }
}

/// Adds the slope from `outline` at `eave` to `top` at `eave + rise`, and dormers on every long enough edge. The walls of dormers go to `walls`, their roofs to `roof`.
pub(super) fn add(
    walls: &mut Mesh,
    roof: &mut Mesh,
    outline: &[[f32; 2]],
    top: &[[f32; 2]],
    eave: f32,
    rise: f32,
) {
    roof.slope(outline, eave, top, eave + rise);
    let n = outline.len();
    for i in 0..n {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let length = (dx * dx + dz * dz).sqrt();
        if length < 6.0 {
            continue;
        }
        let (t, o) = ([dx / length, dz / length], [dz / length, -dx / length]);
        // how far in the slope has moved by mid height, from the matching corner of the top outline
        let inset = ((top[i][0] - a[0]) * -o[0] + (top[i][1] - a[1]) * -o[1]).max(0.0);
        #[expect(clippy::cast_possible_truncation, reason = "a handful of dormers")]
        #[expect(clippy::cast_sign_loss, reason = "the length is at least 6")]
        let count = ((length - 2.0) / 3.5).floor() as usize;
        for k in 0..count {
            #[expect(clippy::cast_precision_loss, reason = "a handful of dormers")]
            let s = length * (k as f32 + 0.5) / count as f32;
            let centre = [
                a[0] + t[0] * s - o[0] * inset * 0.5,
                a[1] + t[1] * s - o[1] * inset * 0.5,
            ];
            dormer(walls, roof, centre, t, o, eave + rise * 0.5);
        }
    }
}

/// One dormer: a small box (1.3 m wide, 1 m deep, 1.4 m high) whose front window faces `o`, centred on the slope at height `mid`.
fn dormer(walls: &mut Mesh, roof: &mut Mesh, c: [f32; 2], t: [f32; 2], o: [f32; 2], mid: f32) {
    let (w, d, y0, y1) = (0.65, 0.5, mid - 0.5, mid + 0.9);
    let at = |s: f32, k: f32| [c[0] + t[0] * s + o[0] * k, c[1] + t[1] * s + o[1] * k];
    // counter-clockwise in (x, z): back left, front left, front right, back right
    let rect = [at(-w, -d), at(-w, d), at(w, d), at(w, -d)];
    let corner = |p: [f32; 2], y: f32| [p[0], y, p[1]];
    // the front is one window cell of the facade texture; the cheeks are a bare wall pixel
    let mut face = |p: [f32; 2], q: [f32; 2], uv: [[f32; 2]; 2]| {
        let (b0, b1, t0, t1) = (corner(p, y0), corner(q, y0), corner(p, y1), corner(q, y1));
        let (lo, hi) = (uv[0], uv[1]);
        walls.triangle([
            (b0, [lo[0], lo[1]]),
            (t1, [hi[0], hi[1]]),
            (b1, [hi[0], lo[1]]),
        ]);
        walls.triangle([
            (b0, [lo[0], lo[1]]),
            (t0, [lo[0], hi[1]]),
            (t1, [hi[0], hi[1]]),
        ]);
    };
    face(rect[1], rect[2], [[0.0, 0.0], [0.25, 1.0 / 3.0]]);
    face(rect[0], rect[1], [[0.0, 0.0], [0.02, 0.02]]);
    face(rect[2], rect[3], [[0.0, 0.0], [0.02, 0.02]]);
    face(rect[3], rect[0], [[0.0, 0.0], [0.02, 0.02]]);
    roof.cap(&rect, y1);
}
