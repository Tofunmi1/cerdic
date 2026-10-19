use bevy_ecs::resource::Resource;
use std::str::FromStr;

/// Which built-in scene to show.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SceneKind {
    /// G4c measurement of 10,000 entities sharing one prototype mesh.
    InstanceProbe,
    /// Editor comparison gallery with the legacy starter's dimensions and poses.
    Gallery,
    /// An empty lit room (floor, walls, sun, warm lights) for looking at authored models.
    Stage,
    /// A cube with a spherical bite, flat shaded, under a sun.
    #[default]
    Kernel,
    /// A grid of spheres sweeping roughness and metalness.
    Materials,
    /// A floor and spheres lit by three coloured point lights.
    Lights,
    /// The kernel scene seen through a mock headset: two eyes side by side.
    Stereo,
    /// A room-scale scene for a real or emulated headset: the cube at eye height, a floor, lights.
    Xr,
    /// A floor with a cube, a sphere and a pillar casting sun shadows onto it and each other.
    Shadows,
    /// Benchmark S1: a room with about 40 props (50k triangles).
    Bench1,
    /// Benchmark S2: a dense room, about 150 props (200k triangles).
    Bench2,
    /// Benchmark S3: a stress room, about 300 props (400k triangles).
    Bench3,
    /// Benchmark S4: about 600 props (one million triangles).
    Bench4,
    /// Benchmark S5: about 1,000 props (two million triangles).
    Bench5,
    /// A glossy dark studio with coloured accent lights, for showing authored models.
    Showcase,
    /// Only a camera: the authored document supplies the sky, sun and ground, so an outdoor scene shows its own sky.
    Open,
    /// Outdoor particle and procedural audio showcase.
    Fx,
}

impl FromStr for SceneKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "instance-probe" => Ok(Self::InstanceProbe),
            "gallery" => Ok(Self::Gallery),
            "stage" => Ok(Self::Stage),
            "" | "kernel" => Ok(Self::Kernel),
            "materials" => Ok(Self::Materials),
            "lights" => Ok(Self::Lights),
            "stereo" => Ok(Self::Stereo),
            "xr" => Ok(Self::Xr),
            "shadows" => Ok(Self::Shadows),
            "bench1" => Ok(Self::Bench1),
            "bench2" => Ok(Self::Bench2),
            "bench3" => Ok(Self::Bench3),
            "bench4" => Ok(Self::Bench4),
            "bench5" => Ok(Self::Bench5),
            "showcase" => Ok(Self::Showcase),
            "open" => Ok(Self::Open),
            "fx" => Ok(Self::Fx),
            _ => Err(()),
        }
    }
}
