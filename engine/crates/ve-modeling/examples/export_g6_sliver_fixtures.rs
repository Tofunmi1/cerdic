//! Export the real lamp shade and goblet meshes used by G6 sliver regression tests.

use std::fmt::Write as FmtWrite;
use std::path::{Path, PathBuf};

use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let programs = crate_dir.join("../../target/reports/big-test/programs");
    let output = crate_dir.join("../ve-geometry/tests/fixtures");
    std::fs::create_dir_all(&output)?;
    export(&programs, &output, "lamp", "shade", "lamp_shade")?;
    export(&programs, &output, "goblet", "goblet", "goblet")?;
    Ok(())
}

fn export(
    programs: &Path,
    output: &Path,
    program: &str,
    expected_part: &str,
    fixture_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(programs.join(format!("{program}.txt")))?;
    let batch = parse_text(&builtin_registry(), &source).map_err(|error| format!("{error:?}"))?;
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g6_fixture")?,
        label: None,
        commands: batch.commands,
    });
    if !matches!(outcome, Outcome::Committed { .. }) {
        return Err(format!("{program} did not commit: {outcome:?}").into());
    }
    let result = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|error| format!("{error:?}"))?;
    let part = result
        .parts
        .iter()
        .find(|part| part.name == expected_part)
        .ok_or_else(|| format!("{program} has no output part named {expected_part:?}"))?;
    write_fixture(output.join(format!("{fixture_name}.poly")), part)?;
    Ok(())
}

fn write_fixture(
    path: PathBuf,
    part: &ve_modeling::Part,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut contents = String::new();
    contents.push_str("VE_POLY_FIXTURE 1\n");
    writeln!(contents, "positions {}", part.positions.len())?;
    for [x, y, z] in &part.positions {
        writeln!(contents, "v {x:.9} {y:.9} {z:.9}")?;
    }
    writeln!(contents, "triangles {}", part.indices.len() / 3)?;
    for triangle in part.indices.chunks_exact(3) {
        writeln!(
            contents,
            "t {} {} {}",
            triangle[0], triangle[1], triangle[2]
        )?;
    }
    std::fs::write(path, contents)?;
    Ok(())
}
