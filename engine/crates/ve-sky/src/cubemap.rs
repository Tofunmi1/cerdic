//! The procedural environment cubemaps for backends that cannot run the atmosphere compute passes (WebGL2): a sky from three colours
//! and a sun disc, the diffuse irradiance map and the roughness mip chain Bevy's environment light reads. Linear RGB, `f32`.
//!
//! Faces are in wgpu's cube layer order: +X, -X, +Y, -Y, +Z, -Z. A texel `(x, y)` of a face of `size` texels has face coordinates
//! `u = 2 (x + 0.5) / size - 1` and `v = 2 (y + 0.5) / size - 1` (row 0 is the top row, `v` grows downward), and its direction is the
//! normalised vector below (the standard cube map table):
//! +X `(1, -v, -u)`, -X `(-1, -v, u)`, +Y `(u, 1, v)`, -Y `(u, -1, -v)`, +Z `(u, -v, 1)`, -Z `(-u, -v, -1)`.

/// The three-colour sky with a sun disc.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkyColors {
    /// Radiance straight up.
    pub zenith: [f32; 3],
    /// Radiance at the horizon.
    pub horizon: [f32; 3],
    /// Radiance straight down.
    pub ground: [f32; 3],
    /// Unit direction from the viewer toward the sun.
    pub sun_direction: [f32; 3],
    /// Radiance added inside the sun disc (zero for no sun).
    pub sun_radiance: [f32; 3],
    /// Angular radius of the sun disc in radians.
    pub sun_angular_radius: f32,
}

/// Six square faces of linear RGBA texels, row-major with row 0 at the top (see the module docs for the order).
#[derive(Clone, Debug, PartialEq)]
pub struct CubeFaces {
    /// Texels along each side of a face.
    pub size: u32,
    /// `faces[f][y * size + x]`.
    pub faces: [Vec<[f32; 4]>; 6],
}

/// The unit direction of texel `(x, y)` on `face` (0 to 5) of a cube of `size` texels, by the table in the module docs.
#[must_use]
#[expect(
    clippy::many_single_char_names,
    clippy::cast_precision_loss,
    reason = "cube face coordinates use conventional u/v notation and the API is defined in f32"
)]
pub fn texel_direction(face: usize, size: u32, x: u32, y: u32) -> [f32; 3] {
    let u = 2.0 * (x as f32 + 0.5) / size as f32 - 1.0;
    let v = 2.0 * (y as f32 + 0.5) / size as f32 - 1.0;
    let d = match face {
        0 => [1.0, -v, -u],
        1 => [-1.0, -v, u],
        2 => [u, 1.0, v],
        3 => [u, -1.0, -v],
        4 => [u, -v, 1.0],
        _ => [-u, -v, -1.0],
    };
    normalize(d)
}

/// The solid angle, in steradians, that texel `(x, y)` of any face covers: `4 / (size^2 (1 + u^2 + v^2)^1.5)` for its face coordinates.
#[must_use]
#[expect(
    clippy::cast_precision_loss,
    reason = "cube coordinates are evaluated in the API's f32 precision"
)]
pub fn texel_solid_angle(size: u32, x: u32, y: u32) -> f32 {
    let u = 2.0 * (x as f32 + 0.5) / size as f32 - 1.0;
    let v = 2.0 * (y as f32 + 0.5) / size as f32 - 1.0;
    4.0 / (size as f32 * size as f32 * (1.0 + u * u + v * v).powf(1.5))
}

/// The sky's radiance toward the unit direction `direction`: for `y >= 0` the horizon colour mixed toward the zenith colour by `sqrt(y)`,
/// for `y < 0` the horizon mixed toward the ground colour by `sqrt(-y)`, plus `sun_radiance` when the angle to `sun_direction` is at most
/// `sun_angular_radius`.
#[must_use]
pub fn sky_radiance(colors: &SkyColors, direction: [f32; 3]) -> [f32; 3] {
    let y = direction[1].clamp(-1.0, 1.0);
    let (a, b, t) = if y >= 0.0 {
        (colors.horizon, colors.zenith, y.sqrt())
    } else {
        (colors.horizon, colors.ground, (-y).sqrt())
    };
    let sun_dot = dot(normalize(direction), normalize(colors.sun_direction));
    let sun = if sun_dot >= colors.sun_angular_radius.cos() {
        colors.sun_radiance
    } else {
        [0.0; 3]
    };
    std::array::from_fn(|i| a[i] * (1.0 - t) + b[i] * t + sun[i])
}

/// The sky sampled at every texel centre, alpha 1.
#[must_use]
pub fn gradient_cubemap(colors: &SkyColors, size: u32) -> CubeFaces {
    CubeFaces {
        size,
        faces: std::array::from_fn(|face| {
            (0..size)
                .flat_map(|y| {
                    (0..size).map(move |x| {
                        let rgb = sky_radiance(colors, texel_direction(face, size, x, y));
                        [rgb[0], rgb[1], rgb[2], 1.0]
                    })
                })
                .collect()
        }),
    }
}

/// The diffuse irradiance map of `source`: each texel, for its direction `n`, holds the cosine-weighted average of the source radiance over the
/// hemisphere around `n`, that is `(1 / pi) * sum(radiance * max(dot(n, l), 0) * solid_angle)`. A white diffuse surface facing `n` reflects
/// exactly that colour, so a uniform sky of radiance `C` gives `C`. `size` is the output face size (8 to 32 is typical).
#[must_use]
pub fn irradiance_cubemap(source: &CubeFaces, size: u32) -> CubeFaces {
    let samples = source_samples(source);
    CubeFaces {
        size,
        faces: std::array::from_fn(|face| {
            let mut texels = Vec::with_capacity((size * size) as usize);
            for y in 0..size {
                for x in 0..size {
                    let n = texel_direction(face, size, x, y);
                    let mut sum = [0.0; 3];
                    for (d, rgb, omega) in &samples {
                        let weight = dot(n, *d).max(0.0) * omega;
                        for c in 0..3 {
                            sum[c] += rgb[c] * weight;
                        }
                    }
                    texels.push([
                        sum[0] / std::f32::consts::PI,
                        sum[1] / std::f32::consts::PI,
                        sum[2] / std::f32::consts::PI,
                        1.0,
                    ]);
                }
            }
            texels
        }),
    }
}

/// A chain of `mip_count` prefiltered copies of `source` for glossy reflections. Mip `i` has face size `max(source.size >> i, 1)`. Mip 0 is the source
/// itself. For `i > 0` each texel is the average of the source radiance (weighted by solid angle) over the cone around its direction with half
/// angle `(pi / 2) * (i / (mip_count - 1))^1.5`.
#[must_use]
#[expect(
    clippy::cast_precision_loss,
    reason = "mip indices define an f32 cone-angle fraction"
)]
pub fn prefiltered_mips(source: &CubeFaces, mip_count: u32) -> Vec<CubeFaces> {
    if mip_count == 0 {
        return Vec::new();
    }
    let samples = source_samples(source);
    let mut result = Vec::with_capacity(mip_count as usize);
    for mip in 0..mip_count {
        let size = (source.size >> mip).max(1);
        if mip == 0 {
            result.push(source.clone());
            continue;
        }
        let half_angle =
            (std::f32::consts::PI / 2.0) * (mip as f32 / (mip_count - 1).max(1) as f32).powf(1.5);
        let threshold = half_angle.cos();
        let faces = std::array::from_fn(|face| {
            let mut texels = Vec::with_capacity((size * size) as usize);
            for y in 0..size {
                for x in 0..size {
                    let n = texel_direction(face, size, x, y);
                    let mut sum = [0.0; 3];
                    let mut total = 0.0;
                    let mut nearest = (f32::NEG_INFINITY, [0.0; 3]);
                    for (d, rgb, omega) in &samples {
                        let alignment = dot(n, *d);
                        if alignment > nearest.0 {
                            nearest = (alignment, *rgb);
                        }
                        if alignment >= threshold {
                            total += omega;
                            for c in 0..3 {
                                sum[c] += rgb[c] * omega;
                            }
                        }
                    }
                    if total > 0.0 {
                        for value in &mut sum {
                            *value /= total;
                        }
                    } else {
                        sum = nearest.1;
                    }
                    texels.push([sum[0], sum[1], sum[2], 1.0]);
                }
            }
            texels
        });
        result.push(CubeFaces { size, faces });
    }
    result
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let length = (dot(v, v)).sqrt();
    if length > 0.0 {
        [v[0] / length, v[1] / length, v[2] / length]
    } else {
        [0.0, 1.0, 0.0]
    }
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn source_samples(source: &CubeFaces) -> Vec<([f32; 3], [f32; 3], f32)> {
    let mut samples = Vec::with_capacity((6 * source.size * source.size) as usize);
    for face in 0..6 {
        for y in 0..source.size {
            for x in 0..source.size {
                let texel = source.faces[face][(y * source.size + x) as usize];
                samples.push((
                    texel_direction(face, source.size, x, y),
                    [texel[0], texel[1], texel[2]],
                    texel_solid_angle(source.size, x, y),
                ));
            }
        }
    }
    samples
}
