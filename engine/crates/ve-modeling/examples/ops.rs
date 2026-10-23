//! The operator reference from the command line, for a model or a person without the MCP tools.
//!
//! ```text
//! cargo run -q -p ve-modeling --example ops                 # every ready operator, one line each
//! cargo run -q -p ve-modeling --example ops -- terrain      # one family (terrain.*)
//! cargo run -q -p ve-modeling --example ops -- xform.noise  # one operator in full: ports, parameters, ranges, defaults
//! cargo run -q -p ve-modeling --example ops -- --search wall # operators whose id or help mentions a word
//! ```

use std::process::ExitCode;

use ve_graph::{OperatorSpec, builtin_registry};

fn first_sentence(text: &str) -> &str {
    text.split_once(". ").map_or(text, |(first, _)| first)
}

fn line(spec: &OperatorSpec) -> String {
    format!("{}  {}", spec.id, first_sentence(spec.doc))
}

fn full(spec: &OperatorSpec) -> String {
    let mut out = format!("{}\n  {}\n", spec.id, spec.doc);
    for input in &spec.inputs {
        out.push_str(&format!(
            "  input {} accepts {:?} ({:?})\n",
            input.name, input.accepts, input.arity
        ));
    }
    out.push_str(&format!("  output {:?}\n", spec.output));
    for param in &spec.params {
        let default = param
            .default
            .as_ref()
            .map_or_else(|| "required".to_owned(), |d| format!("default {d:?}"));
        out.push_str(&format!(
            "  param {} {:?}, {default}: {}\n",
            param.name, param.ty, param.doc
        ));
    }
    out
}

fn kit_line(kit: &ve_modeling::Kit) -> String {
    format!("kit.{}  {}", kit.name, kit.doc)
}

fn main() -> ExitCode {
    let registry = builtin_registry();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut shown = 0_usize;
    match args.as_slice() {
        [] => {
            for spec in registry.ready() {
                println!("{}", line(spec));
                shown += 1;
            }
            for kit in ve_modeling::KITS {
                println!("{}", kit_line(kit));
                shown += 1;
            }
        }
        [wanted] if wanted == "kit" => {
            for kit in ve_modeling::KITS {
                println!("{}", kit_line(kit));
                shown += 1;
            }
        }
        [wanted] if wanted.starts_with("kit.") => {
            // A kit is a group: paste this source ahead of the program, then call it by name.
            if let Some(kit) = ve_modeling::KITS
                .iter()
                .find(|k| format!("kit.{}", k.name) == *wanted)
            {
                println!("{}\n\n{}", kit_line(kit), kit.source);
                shown += 1;
            }
        }
        [flag, word] if flag == "--search" => {
            let word = word.to_lowercase();
            for spec in registry.ready() {
                if spec.id.to_string().to_lowercase().contains(&word)
                    || spec.doc.to_lowercase().contains(&word)
                {
                    println!("{}", line(spec));
                    shown += 1;
                }
            }
            for kit in ve_modeling::KITS {
                if kit.name.contains(&word) || kit.doc.to_lowercase().contains(&word) {
                    println!("{}", kit_line(kit));
                    shown += 1;
                }
            }
        }
        [wanted] => {
            let exact = wanted.contains('.');
            for spec in registry.ready() {
                let id = spec.id.to_string();
                if exact && (id == *wanted || id.split('@').next() == Some(wanted)) {
                    print!("{}", full(spec));
                    shown += 1;
                } else if !exact && id.starts_with(&format!("{wanted}.")) {
                    println!("{}", line(spec));
                    shown += 1;
                }
            }
        }
        _ => {
            eprintln!("usage: ops [family | family.name | --search word]");
            return ExitCode::from(2);
        }
    }
    if shown == 0 {
        eprintln!("nothing matches; run without arguments for every operator");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
