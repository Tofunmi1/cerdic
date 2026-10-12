//! X1 spike: the calls a geometry worker would answer, plus two that misbehave on purpose.

use wasm_bindgen::prelude::*;

use ve_graph::{Alias, Command, CommandBatch, Session, TransactionId};
use ve_kernel::Solid;

fn js<E: std::fmt::Display>(e: E) -> JsError {
    JsError::new(&e.to_string())
}

/// Unions `spheres` overlapping spheres in a row and returns the mesh positions as floats.
#[wasm_bindgen]
pub fn heavy(spheres: u32, segments: u32) -> Result<Vec<f32>, JsError> {
    let mut acc = Solid::sphere(1.0, segments).map_err(js)?;
    for i in 1..spheres {
        let next = Solid::sphere(1.0, segments)
            .map_err(js)?
            .translate([f64::from(i) * 0.7, 0.0, 0.0])
            .map_err(js)?;
        acc = acc.union(&next).map_err(js)?;
    }
    let mesh = acc.to_mesh().map_err(js)?;
    Ok(mesh.positions.iter().flatten().copied().collect())
}

/// Builds a document of `n` boxes through the command path and returns "revision:bytes".
#[wasm_bindgen]
pub fn graph(n: u32) -> Result<String, JsError> {
    let mut session = Session::with_builtin(1);
    let commands: Vec<Command> = (0..n.min(200))
        .map(|_| Command::AddNode {
            op: "solid.box@1".to_owned(),
            params: [(
                "size".to_owned(),
                ve_graph::Value::List(vec![ve_graph::Value::Num(1.0); 3]),
            )]
            .into(),
            label: None,
            alias: None::<Alias>,
        })
        .collect();
    let batch = CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_x1").map_err(js)?,
        label: None,
        commands,
    };
    session.apply(&batch);
    Ok(format!(
        "{}:{}",
        session.document().revision,
        session.document().canonical_json().len()
    ))
}

/// Never returns: stands in for a job stuck inside a kernel call.
#[wasm_bindgen]
pub fn spin() {
    let mut x = 0_u64;
    loop {
        x = std::hint::black_box(x.wrapping_add(1));
    }
}

/// Traps (panic is configured to abort, so this is a wasm `unreachable`).
#[wasm_bindgen]
pub fn trap() {
    panic!("x1 deliberate trap");
}

/// Current wasm linear memory, in bytes.
#[wasm_bindgen]
pub fn memory_bytes() -> f64 {
    (core::arch::wasm32::memory_size(0) * 65_536) as f64
}
