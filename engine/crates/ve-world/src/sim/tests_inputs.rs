//! Programs that read inputs: the library cloth on a hung grid, with a moving anchor and a capsule collider.
use super::*;

/// A `columns` x `rows` grid in the plane z = 0, x from -0.4 to 0.4, y from 0 to 1 (so the top row is at y = 1), with shear diagonals.
fn grid(columns: usize, rows: usize) -> Arc<MeshData> {
    let mut points = Vec::new();
    for r in 0..=rows {
        for c in 0..=columns {
            points.push([-0.4 + 0.8 * c as f32 / columns as f32, 1.0 - r as f32 / rows as f32, 0.0]);
        }
    }
    let at = |c: usize, r: usize| u32::try_from(r * (columns + 1) + c).unwrap();
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

fn cloth(mesh: Arc<MeshData>, params: &[(&str, &[f32])]) -> Sim {
    let program = compile(library::CLOTH).unwrap_or_else(|d| panic!("{d:?}"));
    let mut sim = Sim::with_meshes(program, 1, vec![mesh]).unwrap();
    for (name, values) in params {
        sim.set_param(name, values).unwrap();
    }
    sim
}

fn translate(x: f32, y: f32, z: f32) -> [f32; 16] {
    let mut m = IDENTITY;
    (m[12], m[13], m[14]) = (x, y, z);
    m
}

fn points(s: &Sim) -> Vec<[f32; 3]> {
    s.state("pos").unwrap().chunks(3).map(|c| [c[0], c[1], c[2]]).collect()
}

#[test]
fn the_library_cloth_hangs_from_its_pins_without_stretching_much() {
    let mesh = grid(8, 12);
    let mut s = cloth(Arc::clone(&mesh), &[]);
    s.run(400, 1.0 / 60.0).unwrap();
    let p = points(&s);
    assert!(p.iter().flatten().all(|x| x.is_finite()));
    let top = p[..9].iter().map(|q| q[1]).fold(f32::MAX, f32::min);
    assert!((top - 1.0).abs() < 1e-4, "pinned top row stays: {top}");
    let low = p.iter().map(|q| q[1]).fold(f32::MAX, f32::min);
    assert!(low < 0.1 && low > -0.3, "the cloth hangs about its length (1.0) down from y = 1: lowest {low}");
    // no edge stretched far
    let mut worst = 0.0_f32;
    for v in 0..mesh.len() {
        for &(j, len) in mesh.neighbours(v) {
            let (a, b) = (p[v], p[j as usize]);
            let d = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
            worst = worst.max(d / len);
        }
    }
    assert!(worst < 1.35, "worst edge stretch {worst}");
}

#[test]
fn the_library_cloth_follows_its_anchor_bone_and_is_repeatable() {
    let run = || {
        let mut s = cloth(grid(8, 12), &[]);
        for step in 0..240 {
            let x = (step as f32 / 120.0).min(1.0) * 2.0;
            assert!(s.set_bone("anchor", translate(x, 0.0, 0.0)));
            s.step(1.0 / 60.0).unwrap();
        }
        s
    };
    let s = run();
    let p = points(&s);
    assert!((p[4][0] - 2.0).abs() < 1e-3, "the pinned top row went with the bone: {}", p[4][0]);
    let mean_x = p.iter().map(|q| q[0]).sum::<f32>() / p.len() as f32;
    assert!(mean_x > 1.5 && mean_x < 2.1, "the cloth trails but follows: {mean_x}");
    assert_eq!(s.fingerprint(), run().fingerprint());
}

#[test]
fn a_capsule_collider_pushes_the_cloth_out_of_it() {
    // a vertical capsule behind the cloth's middle (z = -0.05, radius 0.15) from y = 0.3 to 0.7
    let mut s = cloth(grid(8, 12), &[("collider_top", &[0.0, 0.7, -0.05]), ("collider_bottom", &[0.0, 0.3, -0.05]), ("collider_radius", &[0.15])]);
    s.run(300, 1.0 / 60.0).unwrap();
    let p = points(&s);
    let inside = p.iter().filter(|q| {
        let along = q[1].clamp(0.3, 0.7);
        ((q[0]).powi(2) + (q[1] - along).powi(2) + (q[2] + 0.05).powi(2)).sqrt() < 0.15 - 0.02
    });
    assert_eq!(inside.count(), 0, "no point is left inside the capsule");
    assert!(p.iter().any(|q| q[2] > 0.06), "the cloth was pushed forward over the capsule");
}

fn plank_points(s: &Sim) -> Vec<[f32; 3]> {
    points(s)
}

#[test]
fn the_bridge_sags_when_whole_and_falls_in_two_halves_when_cut() {
    let program = compile(library::BRIDGE).unwrap();
    let mut whole = Sim::new(Arc::clone(&program), 1).unwrap();
    whole.run(600, 1.0 / 60.0).unwrap();
    let p = plank_points(&whole);
    assert!(p.iter().flatten().all(|x| x.is_finite()));
    let (first, last, middle) = (p[0], p[40], p[20]);
    assert!((first[1] - 5.0).abs() < 1e-4 && (last[1] - 5.0).abs() < 1e-4, "the posts hold the ends");
    assert!(middle[1] < 3.5 && middle[1] > -1.0,"the middle sags a few metres below the posts: {}", middle[1]);
    assert!(middle[0].abs() < 0.5, "and stays between them: {}", middle[0]);
    // cut the middle link: the halves hang from their posts, nothing stays near the old middle at post height
    let mut cut = Sim::new(program, 1).unwrap();
    cut.run(300, 1.0 / 60.0).unwrap();
    cut.set_param("cut", &[20.0]).unwrap();
    cut.run(600, 1.0 / 60.0).unwrap();
    let q = plank_points(&cut);
    assert!(q[20][1] < middle[1] - 2.0, "the planks at the cut hang lower: {} vs {}", q[20][1], middle[1]);
    assert!(q[20][0] < -0.2 && q[21][0] > 0.2, "the two halves have swung apart: {} {}", q[20][0], q[21][0]);
    // a deterministic result
    let mut again = Sim::new(compile(library::BRIDGE).unwrap(), 1).unwrap();
    again.run(300, 1.0 / 60.0).unwrap();
    again.set_param("cut", &[20.0]).unwrap();
    again.run(600, 1.0 / 60.0).unwrap();
    assert_eq!(cut.fingerprint(), again.fingerprint());
}

#[test]
fn a_push_output_names_the_body_it_pushes_and_is_checked() {
    let program = compile("sim buoy\ninput bone crate\nstate push: vec3[1]\nkernel k each i in push {\n  let depth = 0.0 - bone_point(crate, vec3(0.0)).y\n  push[i] = vec3(0.0, max(depth, 0.0) * 4.0, 0.0)\n}\nschedule k\noutput push crate push\n").unwrap();
    assert_eq!(program.pushes(), vec![("crate", "push")]);
    let mut sim = Sim::new(Arc::clone(&program), 0).unwrap();
    let mut pose = IDENTITY;
    pose[13] = -0.5; // half a metre under water
    assert!(sim.set_bone("crate", pose));
    sim.step(0.1).unwrap();
    assert_eq!(sim.state("push").unwrap(), &[0.0, 2.0, 0.0]);
    let d = compile("sim x\nstate p: f32[1]\nkernel k each i in p { p[i] = 1.0 }\nschedule k\noutput push crate p\n").unwrap_err().remove(0);
    assert!(d.message.contains("vec3"), "{d:?}");
}

#[test]
fn pull_is_the_distance_constraint_move() {
    let mut s = {
        let program = compile("sim x\nstate a: vec3[1]\nkernel k each i in a { a[i] = pull(vec3(0.0), vec3(3.0, 0.0, 0.0), 1.0) }\nschedule k\n").unwrap();
        Sim::new(program, 0).unwrap()
    };
    s.step(0.1).unwrap();
    let a = s.state("a").unwrap();
    assert!((a[0] - 2.0).abs() < 1e-5 && a[1].abs() < 1e-6, "moves two metres toward q so that 1 m remains: {a:?}");
}

#[test]
fn bad_inputs_are_refused_with_a_fix() {
    let program = compile(library::CLOTH).unwrap();
    assert!(Sim::new(Arc::clone(&program), 0).is_err(), "a mesh input needs a mesh");
    let d = compile("sim x\ninput bone b\nstate p: vec3[4]\nkernel k each i in p { p[i] = bone_point(bb, p[i]) }\nschedule k\n").unwrap_err().remove(0);
    assert!(d.suggestion.contains("did you mean `b`"), "{d:?}");
    let d = compile("sim x\ninput mesh m\nstate p: vec3[m]\nkernel k each i in p { for j, l in adjacent(mm, i) { } }\nschedule k\n").unwrap_err().remove(0);
    assert!(d.suggestion.contains("did you mean `m`"), "{d:?}");
}
