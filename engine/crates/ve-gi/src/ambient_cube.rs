//! Our baked probes in the form Bevy's irradiance volumes read: Valve's ambient cube (one colour per axis direction, blended by the squared
//! normal) packed into a 3D texture of `(Rx, 2 Ry, 3 Rz)` texels. See `lib/bevy/crates/bevy_pbr/src/light_probe/irradiance_volume.wesl`
//! for how it is sampled: the sides of one axis are the two halves of the rows (positive first), and the three axes are three blocks along
//! the depth (X, then Y, then Z).

use crate::{GridSpec, ProbeGrid, ProbeSh};

/// The sides of an ambient cube, in the order `ambient_cube` returns them: -X, +X, -Y, +Y, -Z, +Z.
pub const SIDES: usize = 6;

/// An irradiance volume's texels, ready to become a 3D texture.
#[derive(Clone, Debug, PartialEq)]
pub struct AmbientCubeVolume {
    /// Width, height and depth of the texture in texels: `(Rx, 2 Ry, 3 Rz)` for a grid of `(Rx, Ry, Rz)` probes.
    pub size: [u32; 3],
    /// Linear RGBA texels, width first, then height, then depth. Alpha is 1.
    pub texels: Vec<[f32; 4]>,
}

/// The ambient cube of one probe: the colour its irradiance-over-pi has along each of the six axis directions, in the order
/// -X, +X, -Y, +Y, -Z, +Z. Each is `max(dc +/- term, 0)` for the matching axis term (so it is exact along the axes).
#[must_use]
pub fn ambient_cube(sh: &ProbeSh) -> [[f32; 3]; SIDES] {
    let side =
        |term: [f32; 3], sign: f32| std::array::from_fn(|c| (sh.dc[c] + sign * term[c]).max(0.0));
    [
        side(sh.x, -1.0),
        side(sh.x, 1.0),
        side(sh.y, -1.0),
        side(sh.y, 1.0),
        side(sh.z, -1.0),
        side(sh.z, 1.0),
    ]
}

/// The texels of a baked grid as an irradiance volume (see the module docs for the layout).
#[must_use]
pub fn volume_texels(grid: &ProbeGrid) -> AmbientCubeVolume {
    let [rx, ry, rz] = grid.spec.dims.map(|d| d as usize);
    let size = [rx, 2 * ry, 3 * rz];
    let mut texels = vec![[0.0, 0.0, 0.0, 1.0]; size[0] * size[1] * size[2]];
    for z in 0..rz {
        for y in 0..ry {
            for x in 0..rx {
                let cube = ambient_cube(&grid.probes[x + rx * (y + ry * z)]);
                for (side, color) in cube.iter().enumerate() {
                    let (axis, positive) = (side / 2, side % 2 == 1);
                    // The shader (`irradiance_volume.wesl`) reads the second half of the rows for a negative normal component, whatever
                    // the doc comment of `irradiance_volume.rs` says.
                    let t = y + if positive { 0 } else { ry };
                    let p = z + axis * rz;
                    texels[x + size[0] * (t + size[1] * p)] = [color[0], color[1], color[2], 1.0];
                }
            }
        }
    }
    AmbientCubeVolume {
        size: size.map(|d| u32::try_from(d).unwrap_or(u32::MAX)),
        texels,
    }
}

/// Where the unit cube of a Bevy light probe must be put so that its voxel centres land on the grid's probes: `(translation, scale)`
/// in world space. Bevy stretches the volume over a unit cube centred on the origin, with voxel `i` of `R` at `(i + 0.5) / R - 0.5`.
#[must_use]
#[expect(
    clippy::cast_precision_loss,
    reason = "a grid axis holds at most 65 536 probes"
)]
pub fn volume_placement(spec: &GridSpec) -> ([f32; 3], [f32; 3]) {
    let dims = spec.dims.map(|d| d as f32);
    (
        std::array::from_fn(|i| spec.origin[i] + spec.spacing * (dims[i] - 1.0) * 0.5),
        dims.map(|d| spec.spacing * d),
    )
}
