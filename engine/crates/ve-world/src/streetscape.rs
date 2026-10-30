//! Small, deterministic road and street-prop generators for composing urban scenes.

use crate::building::Mesh;

/// Parameters for a straight road module. Coordinates are metres; `at` is the segment centre.
#[derive(Clone, Debug, PartialEq)]
pub struct RoadParams {
    /// Segment centre in metres.
    pub at: [f32; 3],
    /// Segment length along its local X axis, in metres.
    pub length: f32,
    /// Asphalt width in metres.
    pub width: f32,
    /// Number of traffic lanes.
    pub lanes: u8,
    /// Cardinal orientation, `x` or `z`.
    pub axis: String,
    /// Geometry group: asphalt, markings, kerb, or pavement.
    pub part: String,
}

/// Parameters for an individually placeable street prop.
#[derive(Clone, Debug, PartialEq)]
pub struct PropParams {
    /// Prop base position in metres.
    pub at: [f32; 3],
    /// Mast height in metres; fixed-size furniture ignores it.
    pub height: f32,
    /// Cardinal orientation, `x` or `z`.
    pub axis: String,
    /// Street object family.
    pub kind: String,
    /// Geometry group: body, accent, or lens.
    pub part: String,
}

#[derive(Clone, Debug, thiserror::Error)]
/// A road or street-prop parameter that is unsupported or out of range.
pub enum Error {
    #[error("{0}")]
    /// The input was outside the documented generator contract.
    Invalid(&'static str),
}

fn quad(mesh: &mut Mesh, a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3]) {
    let uv = |p: [f32; 3]| [p[0] * 0.25, p[2] * 0.25];
    for (p, uv) in [
        (a, uv(a)),
        (b, uv(b)),
        (c, uv(c)),
        (a, uv(a)),
        (c, uv(c)),
        (d, uv(d)),
    ] {
        let id = u32::try_from(mesh.positions.len()).unwrap_or(0);
        mesh.positions.push(p);
        mesh.indices.push(id);
        mesh.uvs.push(uv);
    }
}

fn cuboid(mesh: &mut Mesh, centre: [f32; 3], size: [f32; 3]) {
    let [x, y, z] = centre;
    let [hx, hy, hz] = [size[0] * 0.5, size[1] * 0.5, size[2] * 0.5];
    let p = [
        [x - hx, y - hy, z - hz],
        [x + hx, y - hy, z - hz],
        [x + hx, y + hy, z - hz],
        [x - hx, y + hy, z - hz],
        [x - hx, y - hy, z + hz],
        [x + hx, y - hy, z + hz],
        [x + hx, y + hy, z + hz],
        [x - hx, y + hy, z + hz],
    ];
    for [a, b, c, d] in [
        [0, 3, 2, 1],
        [4, 5, 6, 7],
        [0, 4, 7, 3],
        [1, 2, 6, 5],
        [0, 1, 5, 4],
        [3, 7, 6, 2],
    ] {
        quad(mesh, p[a], p[b], p[c], p[d]);
    }
}

fn transform(mesh: &mut Mesh, params_at: [f32; 3], axis: &str) {
    for p in &mut mesh.positions {
        let local = *p;
        *p = if axis == "z" {
            [
                params_at[0] + local[2],
                params_at[1] + local[1],
                params_at[2] + local[0],
            ]
        } else {
            [
                params_at[0] + local[0],
                params_at[1] + local[1],
                params_at[2] + local[2],
            ]
        };
    }
}

/// Builds an asphalt segment, lane paint, crosswalks, kerbs, or pavement module.
pub fn build_road(params: &RoadParams) -> Result<Mesh, Error> {
    if !(8.0..=240.0).contains(&params.length) || !(3.0..=36.0).contains(&params.width) {
        return Err(Error::Invalid(
            "road length must be 8–240 m and width 3–36 m",
        ));
    }
    if !(1..=8).contains(&params.lanes) || !["x", "z"].contains(&params.axis.as_str()) {
        return Err(Error::Invalid(
            "road lanes must be 1–8 and axis must be x or z",
        ));
    }
    let mut mesh = Mesh::default();
    let (half_l, half_w) = (params.length * 0.5, params.width * 0.5);
    match params.part.as_str() {
        "asphalt" => quad(
            &mut mesh,
            [-half_l, 0.0, -half_w],
            [-half_l, 0.0, half_w],
            [half_l, 0.0, half_w],
            [half_l, 0.0, -half_w],
        ),
        "markings" => {
            let lane = params.width / f32::from(params.lanes);
            let dash = 2.8_f32;
            let gap = 2.2_f32;
            let mut x = -half_l + 1.0;
            while x + dash < half_l {
                for divider in 1..params.lanes {
                    let z = -half_w + f32::from(divider) * lane;
                    // Leave the road centre open for a double yellow line on two-way streets.
                    if params.lanes % 2 == 0 && divider == params.lanes / 2 {
                        continue;
                    }
                    quad(
                        &mut mesh,
                        [x, 0.025, z - 0.07],
                        [x, 0.025, z + 0.07],
                        [x + dash, 0.025, z + 0.07],
                        [x + dash, 0.025, z - 0.07],
                    );
                }
                x += dash + gap;
            }
            if params.lanes >= 2 {
                for offset in [-0.09, 0.09] {
                    quad(
                        &mut mesh,
                        [-half_l, 0.028, offset - 0.055],
                        [-half_l, 0.028, offset + 0.055],
                        [half_l, 0.028, offset + 0.055],
                        [half_l, 0.028, offset - 0.055],
                    );
                }
            }
        }
        "crosswalk" => {
            let bars = 9_u8;
            let bar_width = 0.38;
            let bar_gap = 0.34;
            let group = f32::from(bars) * bar_width + f32::from(bars - 1) * bar_gap;
            for x_centre in [-half_l + 2.8, half_l - 2.8] {
                for i in 0..bars {
                    let z = -group * 0.5 + f32::from(i) * (bar_width + bar_gap);
                    quad(
                        &mut mesh,
                        [x_centre - 0.22, 0.03, z],
                        [x_centre - 0.22, 0.03, z + bar_width],
                        [x_centre + 0.22, 0.03, z + bar_width],
                        [x_centre + 0.22, 0.03, z],
                    );
                }
            }
        }
        "kerb" => {
            for side in [-1.0, 1.0] {
                cuboid(
                    &mut mesh,
                    [0.0, 0.12, side * (half_w + 0.12)],
                    [params.length, 0.24, 0.24],
                );
            }
        }
        "pavement" => {
            for side in [-1.0, 1.0] {
                cuboid(
                    &mut mesh,
                    [0.0, 0.08, side * (half_w + 1.2)],
                    [params.length, 0.16, 2.2],
                );
            }
        }
        _ => {
            return Err(Error::Invalid(
                "road part must be asphalt, markings, crosswalk, kerb, or pavement",
            ));
        }
    }
    transform(&mut mesh, params.at, &params.axis);
    Ok(mesh)
}

/// Builds one reusable street prop, split into structure, accents, and luminous lenses.
pub fn build_prop(params: &PropParams) -> Result<Mesh, Error> {
    if !(1.0..=18.0).contains(&params.height) || !["x", "z"].contains(&params.axis.as_str()) {
        return Err(Error::Invalid(
            "prop height must be 1–18 m and axis must be x or z",
        ));
    }
    let (mut body, mut accent, mut lens) = (Mesh::default(), Mesh::default(), Mesh::default());
    match params.kind.as_str() {
        "mast" | "streetlight" => {
            cuboid(&mut body, [0.0, 0.12, 0.0], [0.52, 0.24, 0.52]);
            cuboid(
                &mut body,
                [0.0, params.height * 0.5, 0.0],
                [0.16, params.height, 0.16],
            );
            cuboid(
                &mut accent,
                [0.58, params.height - 0.16, 0.0],
                [1.18, 0.12, 0.16],
            );
            cuboid(
                &mut lens,
                [1.12, params.height - 0.24, 0.0],
                [0.48, 0.12, 0.30],
            );
        }
        "traffic_signal" => {
            cuboid(&mut body, [0.0, 0.12, 0.0], [0.5, 0.24, 0.5]);
            cuboid(
                &mut body,
                [0.0, params.height * 0.5, 0.0],
                [0.16, params.height, 0.16],
            );
            cuboid(
                &mut accent,
                [0.44, params.height - 0.8, 0.0],
                [0.7, 0.22, 0.24],
            );
            cuboid(
                &mut body,
                [0.44, params.height - 1.8, 0.0],
                [0.34, 1.65, 0.42],
            );
            for y in [
                params.height - 1.25,
                params.height - 1.8,
                params.height - 2.35,
            ] {
                cuboid(&mut lens, [0.44, y, 0.22], [0.22, 0.22, 0.06]);
            }
        }
        "sign" => {
            cuboid(&mut body, [0.0, 0.12, 0.0], [0.48, 0.24, 0.48]);
            cuboid(
                &mut body,
                [0.0, params.height * 0.5, 0.0],
                [0.12, params.height, 0.12],
            );
            cuboid(
                &mut accent,
                [0.0, params.height - 0.5, 0.0],
                [1.1, 0.85, 0.12],
            );
        }
        "utility_mast" => {
            cuboid(&mut body, [0.0, 0.12, 0.0], [0.8, 0.24, 0.8]);
            cuboid(
                &mut body,
                [0.0, params.height * 0.5, 0.0],
                [0.28, params.height, 0.28],
            );
            cuboid(
                &mut accent,
                [0.0, params.height - 0.45, 0.0],
                [2.8, 0.18, 0.18],
            );
            for x in [-1.0, 0.0, 1.0] {
                cuboid(
                    &mut accent,
                    [x, params.height - 0.68, 0.0],
                    [0.12, 0.30, 0.12],
                );
                cuboid(
                    &mut accent,
                    [x, params.height - 0.48, 0.0],
                    [0.34, 0.08, 0.34],
                );
            }
        }
        "bollard" => {
            cuboid(&mut body, [0.0, 0.12, 0.0], [0.46, 0.24, 0.46]);
            cuboid(&mut body, [0.0, 0.62, 0.0], [0.22, 0.80, 0.22]);
            cuboid(&mut accent, [0.0, 0.94, 0.0], [0.25, 0.12, 0.25]);
        }
        "hydrant" => {
            cuboid(&mut body, [0.0, 0.18, 0.0], [0.62, 0.36, 0.62]);
            cuboid(&mut body, [0.0, 0.78, 0.0], [0.42, 0.92, 0.42]);
            cuboid(&mut body, [0.0, 1.28, 0.0], [0.36, 0.18, 0.36]);
            for side in [-1.0, 1.0] {
                cuboid(&mut accent, [side * 0.28, 0.78, 0.0], [0.32, 0.28, 0.32]);
            }
        }
        "bench" => {
            for z in [-0.45, 0.45] {
                cuboid(&mut body, [0.0, 0.48, z], [1.8, 0.12, 0.12]);
                cuboid(&mut body, [0.0, 0.98, z], [1.8, 0.12, 0.12]);
                cuboid(&mut accent, [0.0, 0.72, z], [1.8, 0.4, 0.08]);
            }
            for x in [-0.72, 0.72] {
                cuboid(&mut body, [x, 0.28, 0.0], [0.12, 0.56, 0.9]);
            }
        }
        "bin" => {
            cuboid(&mut body, [0.0, 0.52, 0.0], [0.58, 1.0, 0.58]);
            cuboid(&mut accent, [0.0, 1.06, 0.0], [0.64, 0.12, 0.64]);
        }
        "bus_shelter" => {
            for x in [-1.6, 1.6] {
                cuboid(&mut body, [x, 1.45, 0.0], [0.12, 2.9, 0.12]);
            }
            cuboid(&mut body, [0.0, 2.92, 0.0], [3.5, 0.16, 1.8]);
            cuboid(&mut accent, [-1.5, 1.5, -0.82], [0.08, 2.7, 0.08]);
            cuboid(&mut accent, [1.5, 1.5, -0.82], [0.08, 2.7, 0.08]);
            cuboid(&mut accent, [0.0, 0.48, 0.0], [2.5, 0.12, 0.55]);
        }
        _ => {
            return Err(Error::Invalid(
                "unknown prop; choose mast, streetlight, traffic_signal, sign, utility_mast, bollard, hydrant, bench, bin, or bus_shelter",
            ));
        }
    }
    let mesh = match params.part.as_str() {
        "body" => &mut body,
        "accent" => &mut accent,
        "lens" => &mut lens,
        _ => return Err(Error::Invalid("prop part must be body, accent, or lens")),
    };
    if mesh.indices.is_empty() {
        return Err(Error::Invalid(
            "this prop has no geometry for the selected part",
        ));
    }
    transform(mesh, params.at, &params.axis);
    Ok(mesh.clone())
}
