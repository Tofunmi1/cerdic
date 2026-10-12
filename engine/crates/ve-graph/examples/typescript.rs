//! Deterministic bridge export. `--check` compares without modifying the checked-in contract.
use std::io::Write;
use ts_rs::{Config, TS};
use ve_graph::{
    Alias, Command, CommandBatch, Diagnostic, DocId, Document, EditorLayer, Flags, IdState,
    InputRef, Material, MaterialId, Node, NodeId, NodeRef, OpId, OutRef, Outcome, Output, PortRef,
    Severity, Span, TransactionId, Value,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = Config::new().with_large_int("number");
    let declarations = [
        Alias::decl(&cfg),
        Command::decl(&cfg),
        CommandBatch::decl(&cfg),
        Diagnostic::decl(&cfg),
        DocId::decl(&cfg),
        Document::decl(&cfg),
        EditorLayer::decl(&cfg),
        Flags::decl(&cfg),
        IdState::decl(&cfg),
        InputRef::decl(&cfg),
        Material::decl(&cfg),
        MaterialId::decl(&cfg),
        Node::decl(&cfg),
        NodeId::decl(&cfg),
        NodeRef::decl(&cfg),
        OpId::decl(&cfg),
        OutRef::decl(&cfg),
        Outcome::decl(&cfg),
        Output::decl(&cfg),
        PortRef::decl(&cfg),
        Severity::decl(&cfg),
        Span::decl(&cfg),
        TransactionId::decl(&cfg),
        Value::decl(&cfg),
    ];
    let mut text =
        String::from("// Generated from ve-graph serde types. Run bun run schema:generate.\n");
    for declaration in declarations {
        text.push_str("export ");
        text.push_str(&declaration);
        text.push('\n');
    }
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../src/engine/generated.ts");
    if std::env::args().any(|a| a == "--check") {
        if std::fs::read_to_string(path).unwrap_or_default() != text {
            return Err("Rust/TypeScript schema drift: run bun run schema:generate".into());
        }
    } else if std::env::args().any(|a| a == "--write") {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, text)?;
    } else {
        std::io::stdout().write_all(text.as_bytes())?;
    }
    Ok(())
}
