//! Writes the GPU plans of a few SimL programs and the CPU reference's states at checkpoints as JSON, for `web/tests/sim-gpu.mjs` to run
//! the shaders on a real WebGPU device and compare. Usage: `cargo run --release -p ve-world --example sim_gpu_dump > dump.json`.
#![expect(clippy::print_stdout, reason = "an example that prints its result")]
use std::sync::Arc;

use ve_world::sim::{GpuInput, GpuStep, IDENTITY, MeshData, Sim, compile, library};

const FALL: &str = "sim fall\nparam g = vec3(0.0, -9.8, 0.0)\nparam count = 100000\nstate pos: vec3[count]\nstate vel: vec3[count]\nkernel init each i in pos { pos[i] = vec3(rand(i, 1.0) * 50.0, 20.0 + rand(i, 2.0) * 10.0, rand(i, 3.0) * 50.0) }\nkernel move each i in pos { vel[i] += g * dt\n pos[i] += vel[i] * dt\n if pos[i].y < 0.0 { pos[i].y = 0.0 - pos[i].y\n vel[i].y = 0.0 - vel[i].y * 0.6 } }\nschedule move\noutput particles pos\n";
const SHIFT: &str = "sim shift\nstate a: f32[1000]\nkernel init each i in a { a[i] = i }\nkernel shift each i in a { a[i] = a[(i + 1) % n] }\nschedule shift\n";
const FLOCK: &str = "sim flock
param count = 1024
param radius = 10.0
param max_speed = 11.0
param cohesion = 0.9
param separation = 4.0
param align = 4.0
state pos: vec3[count]
state vel: vec3[count]
kernel init each i in pos {
  pos[i] = vec3(rand(i, 1.0), rand(i, 2.0), rand(i, 3.0)) * 40.0 - 20.0
  vel[i] = normalize(vec3(rand(i, 4.0), rand(i, 5.0), rand(i, 6.0)) - 0.5) * 6.0
}
kernel flock each i in pos {
  let p = pos[i]
  let v = vel[i]
  let centre = vec3(0.0)
  let heading = vec3(0.0)
  let push = vec3(0.0)
  let seen = 0.0
  for j in neighbors(pos, p, radius) cap 16 {
    if j != i {
      centre += pos[j]
      heading += vel[j]
      let d = p - pos[j]
      push += d / max(dot(d, d), 0.05)
      seen += 1.0
    }
  }
  let acc = vec3(0.0)
  if seen > 0.0 {
    acc = (centre / seen - p) * cohesion + (heading / seen - v) * align + push * separation
  }
  acc -= p * 0.01
  let nv = v + acc * dt
  let sp = length(nv)
  nv = nv * min(1.0, max_speed / max(sp, 0.001))
  nv = nv * max(1.0, 4.0 / max(sp, 0.001))
  vel[i] = nv
}
kernel move each i in pos { pos[i] += vel[i] * dt }
schedule flock; move
output particles pos
";

fn esc(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn schedule(steps: &[GpuStep]) -> String {
    let items: Vec<String> = steps
        .iter()
        .map(|s| match s {
            GpuStep::Run(k) => format!("[\"run\",{k}]"),
            GpuStep::Repeat(n, inner) => format!("[\"repeat\",{n},{}]", schedule(inner)),
        })
        .collect();
    format!("[{}]", items.join(","))
}

fn floats(values: &[f32]) -> String {
    format!("[{}]", values.iter().map(|v| format!("{v:?}")).collect::<Vec<_>>().join(","))
}

/// A `columns` x `rows` grid in the plane z = 0 (x from -0.4 to 0.4, y from 0 to 1) with shear diagonals, as a mesh input.
fn grid(columns: usize, rows: usize) -> Arc<MeshData> {
    let mut points = Vec::new();
    for r in 0..=rows {
        for c in 0..=columns {
            points.push([-0.4 + 0.8 * c as f32 / columns as f32, 1.0 - r as f32 / rows as f32, 0.0]);
        }
    }
    let at = |c: usize, r: usize| u32::try_from(r * (columns + 1) + c).unwrap_or(0);
    let mut edges = Vec::new();
    for r in 0..=rows {
        for c in 0..=columns {
            if c < columns {
                edges.push((at(c, r), at(c + 1, r)));
            }
            if r < rows {
                edges.push((at(c, r), at(c, r + 1)));
            }
            if c < columns && r < rows {
                edges.push((at(c, r), at(c + 1, r + 1)));
                edges.push((at(c + 1, r), at(c, r + 1)));
            }
        }
    }
    MeshData::new(points, &edges)
}

/// The pose of the bone `anchor` at a step: it slides along x by two metres over 120 steps. Every other bone stays where it was.
fn bone_pose(name: &str, step: u32) -> [f32; 16] {
    let mut m = IDENTITY;
    if name == "anchor" {
        m[12] = (step as f32 / 120.0).min(1.0) * 2.0;
    }
    m
}

struct Setup {
    mesh: Option<Arc<MeshData>>,
}

fn program(name: &str, source: &str, dt: f32, checkpoints: &[u32], setup: &Setup) -> Result<String, String> {
    let compiled = compile(source).map_err(|d| format!("{name} does not compile: {d:?}"))?;
    let plan = compiled.gpu_plan().map_err(|e| format!("{name} cannot run on the GPU: {e}"))?;
    let kernels: Vec<String> = plan
        .kernels
        .iter()
        .map(|k| {
            let arrays: Vec<String> = k.arrays.iter().map(|(a, w, s)| format!("[{a},{w},{s}]")).collect();
            let inputs: Vec<String> = k.inputs.iter().map(|b| format!("[{},{},{}]", b.binding, b.input, b.bone)).collect();
            format!("{{\"name\":{},\"wgsl\":{},\"over\":{},\"arrays\":[{}],\"inputs\":[{}]}}", esc(&k.name), esc(&k.wgsl), k.over.map_or("null".to_owned(), |o| o.to_string()), arrays.join(","), inputs.join(","))
        })
        .collect();
    let states: Vec<String> = plan.states.iter().map(|s| format!("{{\"name\":{},\"width\":{},\"count\":{},\"mesh\":{}}}", esc(&s.name), s.width, s.count, s.mesh.map_or("null".to_owned(), |m| m.to_string()))).collect();
    let inputs: Vec<String> = plan.inputs.iter().map(|i| format!("{{\"name\":{},\"bone\":{}}}", esc(&i.name), i.bone)).collect();
    let offsets: Vec<String> = plan.param_offsets.iter().map(|(o, w)| format!("[{o},{w}]")).collect();
    let mut sim = match &setup.mesh {
        Some(mesh) => Sim::with_meshes(compiled, 7, vec![Arc::clone(mesh)]),
        None => Sim::new(compiled, 7),
    }
    .map_err(|e| format!("{name} does not start: {e}"))?;
    let bones: Vec<&GpuInput> = plan.inputs.iter().filter(|i| i.bone).collect();
    let last = checkpoints.last().copied().unwrap_or(0);
    let mut taken = 0;
    let mut marks = Vec::new();
    for &at in checkpoints {
        for step in taken..at {
            for b in &bones {
                sim.set_bone(&b.name, bone_pose(&b.name, step));
            }
            sim.step(dt).map_err(|e| format!("{name} failed: {e}"))?;
        }
        taken = at;
        let arrays: Vec<String> = plan.states.iter().map(|s| format!("{}:{}", esc(&s.name), floats(sim.state(&s.name).unwrap_or(&[])))).collect();
        marks.push(format!("{{\"steps\":{at},\"states\":{{{}}}}}", arrays.join(",")));
    }
    // the poses the host feeds the GPU before each step: 16 f32s per input number, identity for meshes
    let poses: Vec<String> = (0..last)
        .map(|step| {
            let all: Vec<f32> = plan.inputs.iter().flat_map(|i| if i.bone { bone_pose(&i.name, step) } else { IDENTITY }).collect();
            floats(&all)
        })
        .collect();
    let mesh = setup.mesh.as_ref().map_or("null".to_owned(), |m| format!("[{}]", m.gpu_words().iter().map(u32::to_string).collect::<Vec<_>>().join(",")));
    let init = plan.init.map_or("null".to_owned(), |i| i.to_string());
    Ok(format!(
        "{{\"name\":{},\"dt\":{dt:?},\"seed\":7,\"meshWords\":{mesh},\"poses\":[{}],\"plan\":{{\"kernels\":[{}],\"schedule\":{},\"states\":[{}],\"inputs\":[{}],\"params\":{},\"paramOffsets\":[{}],\"init\":{init}}},\"checkpoints\":[{}]}}",
        esc(name),
        poses.join(","),
        kernels.join(","),
        schedule(&plan.schedule),
        states.join(","),
        inputs.join(","),
        floats(&plan.params),
        offsets.join(","),
        marks.join(",")
    ))
}

fn main() -> Result<(), String> {
    let none = Setup { mesh: None };
    let cloth = Setup { mesh: Some(grid(12, 16)) };
    let list = [
        program("fall", FALL, 1.0 / 60.0, &[1, 60, 240], &none)?,
        program("shift", SHIFT, 0.1, &[1, 3], &none)?,
        program("flock", FLOCK, 0.05, &[1, 5, 100], &none)?,
        program("bridge", library::BRIDGE, 1.0 / 60.0, &[1, 60, 300], &none)?,
        program("cloth", library::CLOTH, 1.0 / 60.0, &[1, 60, 240], &cloth)?,
    ];
    println!("{{\"programs\":[{}]}}", list.join(","));
    Ok(())
}
