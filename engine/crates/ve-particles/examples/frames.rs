//! Simulates every preset for four seconds and writes the frames as a JavaScript file a web page can draw.
//!
//!   cargo run -p ve-particles --example frames -- <output.js>
#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "an example reports failures by stopping"
)]
use glam::Mat4;
use std::fmt::Write;
use ve_particles::{Colour, ParticleSystem, Preset};

const PRESETS: [(Preset, &str); 12] = [
    (Preset::Fire, "fire"),
    (Preset::Smoke, "smoke"),
    (Preset::Sparks, "sparks"),
    (Preset::Dust, "dust"),
    (Preset::Rain, "rain"),
    (Preset::Snow, "snow"),
    (Preset::Magic, "magic"),
    (Preset::Splash, "splash"),
    (Preset::Explosion, "explosion"),
    (Preset::Steam, "steam"),
    (Preset::Embers, "embers"),
    (Preset::Fountain, "fountain"),
];

fn main() {
    let path = std::env::args().nth(1).expect("give an output file");
    let mut out = String::from("window.PARTICLES = {");
    for (preset, name) in PRESETS {
        let spec = preset.spec(
            1.0,
            Colour {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
        );
        let mut system = ParticleSystem::new(spec);
        let _ = write!(out, "\"{name}\": [");
        // Thirty frames a second for four seconds; one-shot effects fire again every second so the loop keeps showing them.
        for frame in 0..120 {
            if matches!(preset, Preset::Explosion | Preset::Splash | Preset::Sparks)
                && frame % 30 == 0
            {
                system.burst(150);
            }
            system.step(1.0 / 30.0, Mat4::IDENTITY);
            out.push('[');
            for (i, p) in system.instances().iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                let c = p.colour;
                let _ = write!(
                    out,
                    "[{:.3},{:.3},{:.3},{:.3},{:.2},{:.2},{:.2},{:.2}]",
                    p.position.x, p.position.y, p.position.z, p.size, c.r, c.g, c.b, c.a
                );
            }
            out.push_str("],");
        }
        out.push_str("],");
    }
    out.push_str("};\n");
    std::fs::write(path, out).unwrap();
}
