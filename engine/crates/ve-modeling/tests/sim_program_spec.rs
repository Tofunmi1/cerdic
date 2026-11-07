//! `sim.program`: a simulation written in SimL reaches the part as a description plus the particles' start mesh, and mistakes are refused with the fix.
#![expect(clippy::unwrap_used, clippy::panic, reason = "fixture")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, (String, String)> {
    let commands = parse_text(&builtin_registry(), source).map_err(|e| ("parse".to_owned(), format!("{e:?}")))?.commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_sim").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    Evaluator::default().evaluate(session.document(), Quality::Final).map_err(|e| (e.code.clone(), e.message.clone()))
}

const SWARM: &str = "swarm = sim.program@1(size=0.1, seed=5, source=\"sim swarm\\nparam g = vec3(0.0, -9.8, 0.0)\\nstate pos: vec3[8]\\nstate vel: vec3[8]\\nkernel init each i in pos { pos[i] = vec3(i, 2.0, rand(i, 1.0)) }\\nkernel move each i in pos { vel[i] += g * dt\\n pos[i] += vel[i] * dt }\\nschedule move\\noutput particles pos\\n\")\noutput swarm = swarm\n";

#[test]
fn the_part_carries_the_program_and_one_tetrahedron_per_particle() {
    let result = evaluate(SWARM).unwrap_or_else(|e| panic!("{e:?}"));
    let part = result.parts.iter().find(|p| p.name == "swarm").unwrap();
    let spec = part.sim.as_ref().expect("the part is a simulation");
    assert_eq!((spec.count, spec.seed, spec.particles.as_str()), (8, 5, "pos"));
    assert_eq!(part.indices.len(), 8 * 4 * 3, "four triangles each");
    // particle 3 starts at x = 3: its tetrahedron's corners straddle it by half the size
    let xs: Vec<f32> = part.positions.iter().map(|p| p[0]).collect();
    assert!(xs.iter().any(|x| (x - 3.05).abs() < 1e-4) && xs.iter().any(|x| (x - 2.95).abs() < 1e-4));
}

#[test]
fn a_program_can_drive_a_mesh_and_names_the_bones_it_reads() {
    let text = "grid = poly.grid@1(width=0.8, height=1.0, columns=8, rows=12)\nhung = xform.move@1(grid, by=[0, 0.5, 0])\ncape = sim.program@1(hung, library=\"cloth\")\noutput cape = cape\n";
    let result = evaluate(text).unwrap_or_else(|e| panic!("{e:?}"));
    let part = result.parts.iter().find(|p| p.name == "cape").unwrap();
    let spec = part.sim.as_ref().expect("the cape is a simulation");
    assert_eq!((spec.mode.as_str(), spec.count), ("mesh", 9 * 13));
    assert_eq!(spec.bones, vec!["anchor".to_owned(), "body".to_owned()]);
    assert_eq!(part.positions.len(), 9 * 13, "the drawn mesh is the grid itself");
    let no_mesh = "cape = sim.program@1(library=\"cloth\")\noutput cape = cape\n";
    assert_eq!(evaluate(no_mesh).err().unwrap().0, "graph.missing_input");
}

#[test]
fn mistakes_are_refused_with_line_column_and_fix() {
    let bad = SWARM.replace("pos[i] += vel[i] * dt", "pos[i] += velocity[i] * dt");
    let (code, message) = evaluate(&bad).err().unwrap();
    assert_eq!(code, "sim.compile");
    assert!(message.contains("velocity") && message.contains("line "), "{message}");
    let none = SWARM.replace("output particles pos", "");
    assert_eq!(evaluate(&none).err().unwrap().0, "sim.output");
}

#[test]
fn too_many_particles_are_refused_with_the_fix() {
    let big = SWARM.replace("vec3[8]", "vec3[5000]");
    assert_eq!(evaluate(&big).err().unwrap().0, "budget.sim");
}
