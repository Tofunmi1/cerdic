//! Solid pieces the building generator stands on roofs and walls: a box turned by a yaw, and a short eight-sided drum.
use super::{BAY, Mesh};
use std::f32::consts::TAU;

impl Mesh {
    /// A box turned by `yaw`, resting at height `y`, every face wound outward.
    pub(super) fn block(&mut self, at: [f32; 2], size: [f32; 3], yaw: f32, y: f32) {
        let (s, c) = yaw.sin_cos();
        let corner = |sx: f32, sy: f32, sz: f32| {
            let (lx, lz) = (sx * size[0] * 0.5, sz * size[2] * 0.5);
            [
                at[0] + lx * c - lz * s,
                y + (sy + 1.0) * 0.5 * size[1],
                at[1] + lx * s + lz * c,
            ]
        };
        let centre = [at[0], y + size[1] * 0.5, at[1]];
        let faces: [[[f32; 3]; 4]; 6] = [
            [
                corner(-1.0, -1.0, -1.0),
                corner(1.0, -1.0, -1.0),
                corner(1.0, 1.0, -1.0),
                corner(-1.0, 1.0, -1.0),
            ],
            [
                corner(-1.0, -1.0, 1.0),
                corner(1.0, -1.0, 1.0),
                corner(1.0, 1.0, 1.0),
                corner(-1.0, 1.0, 1.0),
            ],
            [
                corner(-1.0, -1.0, -1.0),
                corner(-1.0, -1.0, 1.0),
                corner(-1.0, 1.0, 1.0),
                corner(-1.0, 1.0, -1.0),
            ],
            [
                corner(1.0, -1.0, -1.0),
                corner(1.0, -1.0, 1.0),
                corner(1.0, 1.0, 1.0),
                corner(1.0, 1.0, -1.0),
            ],
            [
                corner(-1.0, -1.0, -1.0),
                corner(1.0, -1.0, -1.0),
                corner(1.0, -1.0, 1.0),
                corner(-1.0, -1.0, 1.0),
            ],
            [
                corner(-1.0, 1.0, -1.0),
                corner(1.0, 1.0, -1.0),
                corner(1.0, 1.0, 1.0),
                corner(-1.0, 1.0, 1.0),
            ],
        ];
        for face in faces {
            let middle = face.iter().fold([0.0; 3], |s, p| {
                [s[0] + p[0] * 0.25, s[1] + p[1] * 0.25, s[2] + p[2] * 0.25]
            });
            let (e1, e2) = (
                [
                    face[1][0] - face[0][0],
                    face[1][1] - face[0][1],
                    face[1][2] - face[0][2],
                ],
                [
                    face[2][0] - face[0][0],
                    face[2][1] - face[0][1],
                    face[2][2] - face[0][2],
                ],
            );
            let n = [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ];
            let out = [
                middle[0] - centre[0],
                middle[1] - centre[1],
                middle[2] - centre[2],
            ];
            let quad = if n[0] * out[0] + n[1] * out[1] + n[2] * out[2] >= 0.0 {
                [face[0], face[1], face[2], face[3]]
            } else {
                [face[0], face[3], face[2], face[1]]
            };
            let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
            let uv = |k: usize| {
                [
                    middle[0] / BAY + uvs[k][0] * 0.1,
                    middle[2] / BAY + uvs[k][1] * 0.1,
                ]
            };
            self.triangle([(quad[0], uv(0)), (quad[1], uv(1)), (quad[2], uv(2))]);
            self.triangle([(quad[0], uv(0)), (quad[2], uv(2)), (quad[3], uv(3))]);
        }
    }

    /// A short eight-sided drum (a water tank), resting at height `y`.
    pub(super) fn tank(&mut self, at: [f32; 2], radius: f32, height: f32, y: f32) {
        let ring: Vec<[f32; 2]> = (0..8)
            .map(|i| {
                #[expect(clippy::cast_precision_loss, reason = "eight sides")]
                let a = TAU * i as f32 / 8.0;
                [at[0] + radius * a.cos(), at[1] + radius * a.sin()]
            })
            .collect();
        let mut sides = Mesh::default();
        sides.walls(&ring, y, y + height, 3.6);
        // the ring winds counter-clockwise in (x, z) already, so the walls face outward; add the lid
        self.append(&sides);
        self.cap(&ring, y + height);
    }
}
