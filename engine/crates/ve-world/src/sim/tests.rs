use super::*;

fn sim(source: &str) -> Sim {
    let program = compile(source).unwrap_or_else(|d| panic!("compile failed: {d:?}"));
    Sim::new(program, 7).unwrap()
}

fn error_of(source: &str) -> Diagnostic {
    compile(source).expect_err("should be refused").remove(0)
}

const FALL: &str = "
sim fall
param g = vec3(0.0, -9.8, 0.0)
state pos: vec3[4]
state vel: vec3[4]
kernel init each i in pos { pos[i] = vec3(i, 10.0, 0.0) }
kernel move each i in pos {
  vel[i] += g * dt
  pos[i] += vel[i] * dt
}
schedule move
output particles pos
";

#[test]
fn particles_fall_as_semi_implicit_euler_says() {
    let mut s = sim(FALL);
    s.run(60, 1.0 / 60.0).unwrap();
    let pos = s.state("pos").unwrap();
    // y = 10 - g dt^2 n (n + 1) / 2 with n = 60 steps
    let want = 10.0 - 9.8 / 3600.0 * 1830.0;
    for i in 0..4 {
        assert!((pos[i * 3 + 1] - want).abs() < 1e-3, "element {i}: {}", pos[i * 3 + 1]);
        assert!((pos[i * 3] - i as f32).abs() < 1e-6, "x stays where init put it");
    }
    assert_eq!(s.program.outputs(), vec![("particles", "pos")]);
}

#[test]
fn a_kernel_reads_the_state_as_it_was_when_it_started() {
    let mut s = sim("
sim shift
state a: f32[4]
kernel init each i in a { a[i] = i }
kernel shift each i in a { a[i] = a[(i + 1) % n] }
schedule shift
");
    s.step(0.1).unwrap();
    // in-place order would give [1, 2, 3, 1]
    assert_eq!(s.state("a").unwrap(), &[1.0, 2.0, 3.0, 0.0]);
}

#[test]
fn the_same_program_gives_the_same_bits_and_reset_restores_it() {
    let mut a = sim(FALL);
    let mut b = sim(FALL);
    a.run(100, 0.01).unwrap();
    b.run(100, 0.01).unwrap();
    assert_eq!(a.fingerprint(), b.fingerprint());
    let moved = a.fingerprint();
    a.reset().unwrap();
    assert_ne!(a.fingerprint(), moved);
    assert_eq!(a.fingerprint(), sim(FALL).fingerprint());
}

#[test]
fn parameters_can_be_set_and_are_checked() {
    let mut s = sim(FALL);
    s.set_param("g", &[0.0, 0.0, 0.0]).unwrap();
    s.run(10, 0.1).unwrap();
    assert!((s.state("pos").unwrap()[1] - 10.0).abs() < 1e-6, "no gravity, no fall");
    assert!(s.set_param("g", &[1.0]).is_err());
    assert!(s.set_param("nope", &[1.0]).is_err());
}

#[test]
fn mistakes_are_refused_with_a_line_and_a_fix() {
    let d = error_of("sim x\nstate pos: vec3[4]\nkernel k each i in pos {\n  pos[i] = psoition\n}\nschedule k\n");
    assert_eq!(d.line, 4);
    assert!(d.message.contains("psoition"), "{d:?}");
    let d = error_of("sim x\nstate pos: vec3[4]\nkernel k each i in pos {\n  pos[i] = 1.0\n}\nschedule k\n");
    assert_eq!(d.line, 4);
    assert!(d.message.contains("vec3"), "{d:?}");
    let d = error_of("sim x\nstate pos: vec3[4]\nkernel k each i in pos {\n  for j in neighbors(pos, pos[i], 1.0) {\n  }\n}\nschedule k\n");
    assert!(d.suggestion.contains("cap"), "{d:?}");
    let d = error_of("sim x\nstate pos: vec3[4]\nkernel k each i in pos {\n  pos[i] = vec3(1.0, 2.0)\n}\nschedule k\n");
    assert!(d.message.contains("vec3 needs 3"), "{d:?}");
    let d = error_of("sim x\nstate pos: vec3[4]\nkernel k each i in pos {\n  pos[i] = vec3(1.0)\n}\n");
    assert!(d.message.contains("schedule"), "{d:?}");
    let d = error_of("sim x\nstate pos: vec3[4]\nkernel move each i in pos {\n  pos[i] = vec3(1.0)\n}\nschedule mvoe\n");
    assert!(d.suggestion.contains("move"), "{d:?}");
}

#[test]
fn indices_and_writes_outside_the_element_are_runtime_errors() {
    let mut s = sim("sim x\nstate a: f32[4]\nkernel k each i in a { a[i + 1] = 1.0 }\nschedule k\n");
    let e = s.step(0.1).unwrap_err();
    assert!(e.message.contains("own element"), "{e}");
    let mut s = sim("sim x\nstate a: f32[4]\nkernel k each i in a { a[i] = a[i + 9] }\nschedule k\n");
    assert!(s.step(0.1).unwrap_err().message.contains("outside"));
    let mut s = sim("sim x\nstate a: f32[1]\nkernel k each i in a {\n for p in 0..4000 {\n for q in 0..4000 {\n a[i] += 1.0\n }\n }\n}\nschedule k\n");
    assert!(s.step(0.1).unwrap_err().message.contains("statements"));
}

#[test]
fn state_sizes_are_capped() {
    let program = compile("sim x\nparam big = 2000000.0\nstate a: f32[big]\nkernel k each i in a { a[i] = 1.0 }\nschedule k\n").unwrap();
    assert!(Sim::new(program, 0).is_err());
}

#[test]
fn the_neighbour_grid_finds_exactly_what_testing_every_element_finds() {
    let mut s = sim("
sim count
state pos: vec3[500]
state cnt: f32[500]
kernel init each i in pos { pos[i] = vec3(rand(i, 1.0), rand(i, 2.0), rand(i, 3.0)) * 30.0 - 15.0 }
kernel count each i in pos {
  let c = 0.0
  for j in neighbors(pos, pos[i], 4.0) cap 4096 { c += 1.0 }
  cnt[i] = c
}
schedule count
");
    s.step(0.1).unwrap();
    let pos = s.state("pos").unwrap().to_vec();
    let cnt = s.state("cnt").unwrap();
    for (i, p) in pos.chunks(3).enumerate() {
        let want = pos.chunks(3).filter(|q| (0..3).map(|k| (p[k] - q[k]).powi(2)).sum::<f32>() <= 16.0).count();
        assert!((cnt[i] - want as f32).abs() < 1.5, "element {i}: grid {} vs all {want}", cnt[i]);
    }
    assert!(cnt.iter().any(|c| *c > 1.0), "some elements have neighbours");
}

#[test]
fn a_big_flock_runs_fast_enough_because_of_the_grid() {
    let source = MURMURATION.replace("param count = 256", "param count = 4000");
    let mut s = sim(&source);
    let start = std::time::Instant::now();
    s.run(5, 0.05).unwrap();
    assert!(start.elapsed().as_secs_f32() < 20.0, "5 steps of 4000 birds took {:?}", start.elapsed());
}

pub(super) const MURMURATION: &str = "
sim murmuration
param count = 256
param radius = 10.0
param max_speed = 12.0
param cohesion = 0.8
param separation = 3.0
param align = 4.0
state pos: vec3[count]
state vel: vec3[count]
kernel init each i in pos {
  pos[i] = vec3(rand(i, 1.0), rand(i, 2.0), rand(i, 3.0)) * 24.0 - 12.0
  vel[i] = normalize(vec3(rand(i, 4.0), rand(i, 5.0), rand(i, 6.0)) - 0.5) * 6.0
}
kernel flock each i in pos {
  let p = pos[i]
  let v = vel[i]
  let centre = vec3(0.0)
  let heading = vec3(0.0)
  let push = vec3(0.0)
  let seen = 0.0
  for j in neighbors(pos, p, radius) cap 24 {
    if j != i {
      centre += pos[j]
      heading += vel[j]
      let d = p - pos[j]
      push += d / max(dot(d, d), 0.01)
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

fn polarisation(s: &Sim) -> f32 {
    let vel = s.state("vel").unwrap();
    let mut sum = [0.0_f32; 3];
    for v in vel.chunks(3) {
        let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        for k in 0..3 {
            sum[k] += v[k] / len;
        }
    }
    let n = (vel.len() / 3) as f32;
    (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt() / n
}

#[test]
fn a_flock_with_neighbour_rules_aligns_stays_finite_and_is_repeatable() {
    let mut s = sim(MURMURATION);
    let before = polarisation(&s);
    s.run(400, 0.05).unwrap();
    let after = polarisation(&s);
    assert!(s.state("pos").unwrap().iter().all(|x| x.is_finite() && x.abs() < 500.0), "nothing flew off");
    assert!(before < 0.2, "random headings at the start: {before}");
    assert!(after > 0.5, "the flock aligned: {before} -> {after}");
    let mut again = sim(MURMURATION);
    again.run(400, 0.05).unwrap();
    assert_eq!(s.fingerprint(), again.fingerprint());
}
