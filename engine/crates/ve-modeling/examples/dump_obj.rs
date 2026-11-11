//! Builds a program written in the text language and writes every part's triangles as a Wavefront OBJ file, for tools outside the engine
//! (a remesher spike, a viewer). Instanced parts are written once, untransformed.
//!
//! `cargo run -p ve-modeling --example dump_obj -- program.txt out_dir`
#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::unwrap_used,
    reason = "a command line tool prints its report and stops at its own usage errors"
)]
use std::fmt::Write as _;
use std::process::ExitCode;

use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (Some(program), Some(directory)) = (args.next(), args.next()) else {
        eprintln!("usage: dump_obj program.txt out_dir");
        return ExitCode::from(2);
    };
    let source = std::fs::read_to_string(&program).unwrap();
    let commands = match parse_text(&builtin_registry(), &source) {
        Ok(parsed) => parsed.commands,
        Err(error) => {
            eprintln!("PARSE ERROR {error:?}");
            return ExitCode::FAILURE;
        }
    };
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_dump_obj").unwrap(),
        label: None,
        commands,
    });
    if !matches!(applied, Outcome::Committed { .. }) {
        eprintln!("REFUSED {applied:?}");
        return ExitCode::FAILURE;
    }
    let result = match Evaluator::default().evaluate(session.document(), Quality::Final) {
        Ok(result) => result,
        Err(error) => {
            eprintln!("EVALUATION FAILED {}: {}", error.code, error.message);
            return ExitCode::FAILURE;
        }
    };
    std::fs::create_dir_all(&directory).unwrap();
    let mut names: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut sidecar: Vec<serde_json::Value> = Vec::new();
    for part in &result.parts {
        if part.positions.is_empty() {
            continue;
        }
        let mut text = String::new();
        // By default a placed set is written once, untransformed. With VE_DUMP_INSTANCES set every placed copy is written where it stands (a picture of the
        // scene wants the pillars, bolts and lugs in place); copies are capped so a scattered forest does not write gigabytes.
        let identity = [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];
        let copies: Vec<[f32; 16]> = match (&part.transforms, std::env::var_os("VE_DUMP_INSTANCES"))
        {
            (Some(placed), Some(_)) if placed.len() * part.positions.len() <= 4_000_000 => {
                placed.clone()
            }
            _ => vec![identity],
        };
        for m in &copies {
            for p in &part.positions {
                // column-major: the translation is the last column
                let x = m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12];
                let y = m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13];
                let z = m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14];
                writeln!(text, "v {x} {y} {z}").unwrap();
            }
        }
        for copy in 0..copies.len() {
            let base = u32::try_from(copy * part.positions.len()).unwrap();
            for t in part.indices.chunks_exact(3) {
                writeln!(
                    text,
                    "f {} {} {}",
                    base + t[0] + 1,
                    base + t[1] + 1,
                    base + t[2] + 1
                )
                .unwrap();
            }
        }
        // several materials of one output are several parts with the same name: the later ones get `name__2.obj`, `name__3.obj`
        let seen = names.entry(part.name.clone()).or_insert(0);
        *seen += 1;
        let path = if *seen == 1 {
            format!("{directory}/{}.obj", part.name)
        } else {
            format!("{directory}/{}__{}.obj", part.name, *seen)
        };
        std::fs::write(&path, text).unwrap();
        // The sidecar a renderer needs to look like the scene and to play it: the surface, the tracks and the parent of each part.
        let file = std::path::Path::new(&path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default()
            .to_owned();
        let material = part.material.as_ref().map(|m| {
            serde_json::json!({
                "color": m.color, "opacity": m.opacity, "metallic": m.metallic,
                "roughness": m.roughness, "emissive": m.emissive,
            })
        });
        sidecar.push(serde_json::json!({
            "file": file,
            "name": part.name,
            "parent": part.parent,
            "material": material,
            "animation": serde_json::to_value(&part.animation).unwrap(),
        }));
        println!(
            "{path}: {} vertices, {} triangles",
            part.positions.len(),
            part.indices.len() / 3
        );
    }
    std::fs::write(
        format!("{directory}/parts.json"),
        serde_json::to_string(&sidecar).unwrap(),
    )
    .unwrap();
    ExitCode::SUCCESS
}
