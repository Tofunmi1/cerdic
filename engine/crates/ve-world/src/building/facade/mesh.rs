//! Deterministic facade mesh helpers shared by the building grammar.

use super::super::Mesh;

pub(super) fn point(a: [f32; 2], b: [f32; 2], u: f32, y: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * u, y, a[1] + (b[1] - a[1]) * u]
}

fn face(mesh: &mut Mesh, a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3]) {
    let uv = |p: [f32; 3]| [p[0] / 2.0, p[1] / 2.0];
    mesh.triangle([(a, uv(a)), (b, uv(b)), (c, uv(c))]);
    mesh.triangle([(a, uv(a)), (c, uv(c)), (d, uv(d))]);
}

/// A quad whose triangles face `outward`. The four points may be supplied in either winding;
/// facade edges run clockwise and counter-clockwise around a building, so callers cannot use one
/// fixed order and still get outward normals on every side.
pub(super) fn face_outward(mesh: &mut Mesh, corners: [[f32; 3]; 4], outward: [f32; 3]) {
    let [a, b, c, d] = corners;
    let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let normal = [
        e1[1] * e2[2] - e1[2] * e2[1],
        e1[2] * e2[0] - e1[0] * e2[2],
        e1[0] * e2[1] - e1[1] * e2[0],
    ];
    if normal[0] * outward[0] + normal[1] * outward[1] + normal[2] * outward[2] < 0.0 {
        face(mesh, d, c, b, a);
    } else {
        face(mesh, a, b, c, d);
    }
}

pub(super) fn wall_panel(
    mesh: &mut Mesh,
    a: [f32; 2],
    b: [f32; 2],
    u0: f32,
    u1: f32,
    y0: f32,
    y1: f32,
) {
    let outside = outside(a, b);
    face_outward(
        mesh,
        [
            point(a, b, u0, y0),
            point(a, b, u0, y1),
            point(a, b, u1, y1),
            point(a, b, u1, y0),
        ],
        [outside[0], 0.0, outside[1]],
    );
}

pub(super) fn outside(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    let dx = b[0] - a[0];
    let dz = b[1] - a[1];
    let length = (dx * dx + dz * dz).sqrt().max(1e-6);
    [dz / length, -dx / length]
}

/// Adds an edge-aligned prism without angle-based trigonometry. Rectangular building edges have
/// exact cardinal tangents, keeping generated mesh coordinates identical in native and WASI.
pub(super) fn block_on_edge(
    mesh: &mut Mesh,
    a: [f32; 2],
    b: [f32; 2],
    u: f32,
    offset: f32,
    size: [f32; 3],
    bottom: f32,
) {
    let dx = b[0] - a[0];
    let dz = b[1] - a[1];
    let length = (dx * dx + dz * dz).sqrt().max(1e-6);
    let tangent = [dx / length, dz / length];
    let normal = outside(a, b);
    let center = [
        a[0] + dx * u + normal[0] * offset,
        a[1] + dz * u + normal[1] * offset,
    ];
    let corner = |along: f32, out: f32, y: f32| {
        [
            center[0] + tangent[0] * along + normal[0] * out,
            y,
            center[1] + tangent[1] * along + normal[1] * out,
        ]
    };
    let (half_width, half_depth, top) = (size[0] * 0.5, size[2] * 0.5, bottom + size[1]);
    let outward = [normal[0], 0.0, normal[1]];
    let inward = [-normal[0], 0.0, -normal[1]];
    let right = [tangent[0], 0.0, tangent[1]];
    let left = [-tangent[0], 0.0, -tangent[1]];
    face_outward(
        mesh,
        [
            corner(-half_width, half_depth, bottom),
            corner(-half_width, half_depth, top),
            corner(half_width, half_depth, top),
            corner(half_width, half_depth, bottom),
        ],
        outward,
    );
    face_outward(
        mesh,
        [
            corner(half_width, -half_depth, bottom),
            corner(half_width, -half_depth, top),
            corner(-half_width, -half_depth, top),
            corner(-half_width, -half_depth, bottom),
        ],
        inward,
    );
    face_outward(
        mesh,
        [
            corner(half_width, -half_depth, bottom),
            corner(half_width, half_depth, bottom),
            corner(half_width, half_depth, top),
            corner(half_width, -half_depth, top),
        ],
        right,
    );
    face_outward(
        mesh,
        [
            corner(-half_width, half_depth, bottom),
            corner(-half_width, -half_depth, bottom),
            corner(-half_width, -half_depth, top),
            corner(-half_width, half_depth, top),
        ],
        left,
    );
    face_outward(
        mesh,
        [
            corner(-half_width, -half_depth, top),
            corner(half_width, -half_depth, top),
            corner(half_width, half_depth, top),
            corner(-half_width, half_depth, top),
        ],
        [0.0, 1.0, 0.0],
    );
    face_outward(
        mesh,
        [
            corner(-half_width, half_depth, bottom),
            corner(half_width, half_depth, bottom),
            corner(half_width, -half_depth, bottom),
            corner(-half_width, -half_depth, bottom),
        ],
        [0.0, -1.0, 0.0],
    );
}

pub(super) fn roof_tank(mesh: &mut Mesh, at: [f32; 2], radius: f32, height: f32, y: f32) {
    const DIAGONAL: f32 = 0.707_106_77;
    let diagonal = radius * DIAGONAL;
    let ring = [
        [at[0] + radius, at[1]],
        [at[0] + diagonal, at[1] + diagonal],
        [at[0], at[1] + radius],
        [at[0] - diagonal, at[1] + diagonal],
        [at[0] - radius, at[1]],
        [at[0] - diagonal, at[1] - diagonal],
        [at[0], at[1] - radius],
        [at[0] + diagonal, at[1] - diagonal],
    ];
    mesh.walls(&ring, y, y + height, 3.6);
    mesh.cap(&ring, y + height);
}

pub(super) fn outline(
    width: f32,
    depth: f32,
    floor: u8,
    floors: u8,
    stepped: bool,
) -> Vec<[f32; 2]> {
    let steps = if stepped {
        floor.saturating_sub(floors.saturating_mul(3) / 4)
    } else {
        0
    };
    let inset = f32::from(steps) * 0.42;
    let (x, z) = (
        (width * 0.5 - inset).max(3.0),
        (depth * 0.5 - inset).max(3.0),
    );
    vec![[-x, -z], [x, -z], [x, z], [-x, z]]
}
