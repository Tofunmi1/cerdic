//! The GPU code generator: every shader it writes must parse and validate, and what it cannot do is refused with a reason.
use super::*;

fn validate(name: &str, wgsl: &str) {
    let module = naga::front::wgsl::parse_str(wgsl).unwrap_or_else(|e| panic!("{name}: {}\n{wgsl}", e.emit_to_string(wgsl)));
    naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
        .validate(&module)
        .unwrap_or_else(|e| panic!("{name}: validation failed: {e:?}\n{wgsl}"));
}

fn plan_of(source: &str) -> GpuPlan {
    compile(source).unwrap().gpu_plan().unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn every_kernel_of_the_test_programs_becomes_valid_wgsl() {
    let programs: [(&str, &str); 5] = [
        ("fall", "sim fall\nparam g = vec3(0.0, -9.8, 0.0)\nstate pos: vec3[4]\nstate vel: vec3[4]\nkernel init each i in pos { pos[i] = vec3(i, 10.0, 0.0) }\nkernel move each i in pos { vel[i] += g * dt\n pos[i] += vel[i] * dt }\nschedule move\noutput particles pos\n"),
        ("shift", "sim shift\nstate a: f32[4]\nkernel init each i in a { a[i] = i }\nkernel shift each i in a { a[i] = a[(i + 1) % n] }\nschedule shift\n"),
        ("rain", "sim rain\nparam speed = 9.0\nstate pos: vec3[100]\nkernel init each i in pos { pos[i] = vec3(rand(i, 1.0) * 20.0, rand(i, 2.0) * 30.0, rand(i, 3.0) * 20.0) }\nkernel fall each i in pos {\n  pos[i].y -= speed * dt\n  if pos[i].y < 0.0 { pos[i] = vec3(rand(i, step) * 20.0, 30.0, rand(i, step + 1.0) * 20.0) }\n}\nschedule fall\noutput particles pos\n"),
        ("mix", "sim mix\nstate v: vec3[8]\nstate b: bool[8]\nkernel k each i in v {\n  let x = vec3(1.0, 2.0, 3.0) * 0.5\n  x *= 2.0\n  let s = smoothstep(0.0, 1.0, x)\n  let c = clamp(x, 0.0, 1.0) + mix(x, s, 0.5) + normalize(x) + cross(x, s)\n  let m = max(min(length(c), 3.0), sqrt(dot(x, s)) + pow(abs(c.x), 2.0) + exp(c.y) + log(c.z) + sin(c.x) + atan2(c.y, c.z))\n  for k in 0..3 { m += select(true, 1.0, 2.0) + hash(i, k) + (c.x % 0.3) / (c.y + 0.1) }\n  if m > 1.0 && !(m < 0.0) || i == 3.0 { v[i] = c } else { v[i] = vec3(m) }\n  b[i] = m > 2.0\n}\nschedule k\n"),
        ("flock", super::tests::MURMURATION),
    ];
    for (name, source) in programs {
        let plan = plan_of(source);
        assert!(!plan.kernels.is_empty());
        for k in &plan.kernels {
            validate(&format!("{name}/{}", k.name), &k.wgsl);
        }
    }
}

#[test]
fn the_plan_says_which_arrays_a_kernel_writes_and_how_the_schedule_runs() {
    let plan = plan_of("sim x\nstate a: f32[4]\nstate b: f32[4]\nkernel init each i in a { a[i] = 1.0 }\nkernel copy each i in b { b[i] = a[i] * 2.0 }\nkernel bump each i in a { a[i] += b[i] }\nschedule copy; repeat 3 { bump }\n");
    assert_eq!(plan.init, Some(0));
    let copy = &plan.kernels[1];
    assert_eq!(copy.arrays, vec![(0, false, false), (1, true, true)], "reads a (live, read-only), writes b (live + snapshot)");
    let bump = &plan.kernels[2];
    assert_eq!(bump.arrays, vec![(0, true, true), (1, false, false)]);
    assert!(matches!(&plan.schedule[..], [GpuStep::Run(1), GpuStep::Repeat(3, inner)] if matches!(inner[..], [GpuStep::Run(2)])));
}

#[test]
fn the_library_programs_become_valid_wgsl_with_their_inputs_bound() {
    let cloth = plan_of(library::CLOTH);
    for k in &cloth.kernels {
        validate(&format!("cloth/{}", k.name), &k.wgsl);
    }
    // the cloth's states are sized by its mesh, and the solve kernel reads the bone poses and the mesh
    assert_eq!(cloth.states[0].mesh, Some(0));
    assert_eq!(cloth.inputs.iter().map(|i| (i.name.as_str(), i.bone)).collect::<Vec<_>>(), vec![("cloth", false), ("anchor", true), ("body", true)]);
    let solve = cloth.kernels.iter().find(|k| k.name == "solve").unwrap();
    assert!(solve.inputs.iter().any(|b| b.bone) && solve.inputs.iter().any(|b| !b.bone), "{:?}", solve.inputs);
    let bridge = plan_of(library::BRIDGE);
    for k in &bridge.kernels {
        validate(&format!("bridge/{}", k.name), &k.wgsl);
    }
}
