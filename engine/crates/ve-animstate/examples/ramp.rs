//! Runs the locomotion state machine through a scripted walk (stand, speed up to a run, jump, slow down) and writes what it decides every
//! twentieth of a second as a JavaScript file a web page can draw.
//!
//!   cargo run -p ve-animstate --example ramp -- <output.js>
#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "an example reports failures by stopping"
)]
use std::fmt::Write;
use ve_animstate::{Machine, Preset};

/// The speed in metres per second at time `t`: stand, accelerate to a run, hold, slow to a walk, stop.
fn speed(t: f32) -> f32 {
    match t {
        t if t < 1.0 => 0.0,
        t if t < 4.0 => (t - 1.0) / 3.0 * 4.5,
        t if t < 7.0 => 4.5,
        t if t < 10.0 => 4.5 - (t - 7.0) / 3.0 * 3.3,
        t if t < 11.0 => 1.2 - (t - 10.0) * 1.2,
        _ => 0.0,
    }
}

fn main() {
    let path = std::env::args().nth(1).expect("give an output file");
    let preset = Preset::Locomotion;
    let mut machine = Machine::new(preset.graph(), &preset.clip_lengths());
    let mut out = String::from("window.RAMP = [");
    let dt = 1.0 / 60.0;
    let mut jumped = false;
    for step in 0..(13 * 60) {
        let t = f32::from(u16::try_from(step).unwrap()) * dt;
        machine.set_float("speed", speed(t));
        machine.set_bool("grounded", true);
        if !jumped && t >= 8.0 {
            machine.trigger("jump");
            jumped = true;
        }
        let pose = machine.step(dt);
        if step % 3 == 0 {
            let state = machine.current_state().unwrap_or("");
            let _ = write!(
                out,
                "{{\"t\":{t:.2},\"speed\":{:.2},\"state\":\"{state}\",\"w\":{{",
                speed(t)
            );
            for (i, layer) in pose.layers.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                let name = if layer.clip.is_empty() {
                    "walk"
                } else {
                    layer.clip.as_str()
                };
                let _ = write!(out, "\"{name}\":{:.3}", layer.weight);
            }
            out.push_str("}},");
        }
    }
    out.push_str("];\n");
    std::fs::write(path, out).unwrap();
}
