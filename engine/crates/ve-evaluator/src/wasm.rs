//! WebAssembly entry points used by the modeling worker.
use serde_json::json;
use std::cell::RefCell;
use ve_modeling::{Evaluation, Evaluator, Quality};
use wasm_bindgen::prelude::*;

pub mod morph;

/// Prints a document snapshot as compact text, or standalone compact node lines when `nodes` is true.
#[wasm_bindgen]
#[must_use]
pub fn doc_text_compact(document: &str, nodes: bool) -> String {
    crate::text::compact(document, nodes)
}

/// Parses text against the page's candidate document, including pipes, loops and existing labels.
#[wasm_bindgen]
#[must_use]
pub fn doc_parse_text(document: &str, text: &str) -> String {
    crate::text::parse(document, text)
}

/// Runs one script under the default limits (`polls` overrides the work budget when above zero) and answers `{"ok": text}` or
/// `{"error": {message, line, column, stopped}}`. The page stops a script that must end sooner by ending the worker.
#[wasm_bindgen]
#[must_use]
#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "the page passes small whole numbers")]
pub fn script_run(source: &str, seed: f64, polls: f64) -> String {
    use ve_modeling::script::{Limits, run};
    let mut limits = Limits::default();
    if polls > 0.0 {
        limits.polls = polls as u64;
    }
    match run(source, seed as u32, &limits, std::rc::Rc::new(|| false)) {
        Ok(text) => json!({"ok": text}).to_string(),
        Err(e) => json!({"error": {"message": e.message, "line": e.line, "column": e.column, "stopped": e.stopped}}).to_string(),
    }
}

#[derive(Default)]
struct Host {
    evaluator: Evaluator,
    result: Option<Evaluation>,
}
thread_local! { static HOST: RefCell<Host> = RefCell::new(Host::default()); }

/// Hands the evaluator the human body pack (`human.pack`) for `body.human`; returns an empty string, or what is wrong with the bytes.
#[wasm_bindgen]
#[must_use]
pub fn human_pack_set(bytes: &[u8]) -> String {
    ve_modeling::set_human_pack(bytes).err().unwrap_or_default()
}

/// Preflights and begins a candidate; bad input is a diagnostic value.
#[wasm_bindgen]
#[must_use]
pub fn eval_begin(document: &str, quality: &str) -> String {
    if document.len() > ve_graph::MAX_DOCUMENT_BYTES {
        return json!({"error":{"code":"budget.source","message":"document exceeds 2 MiB"}})
            .to_string();
    }
    let quality = match quality {
        "preview" => Quality::Preview,
        "final" => Quality::Final,
        _ => {
            return json!({"error":{"code":"eval.quality","message":"invalid quality"}})
                .to_string();
        }
    };
    let doc = match serde_json::from_str::<ve_graph::Document>(document) {
        Ok(doc) => doc,
        Err(error) => {
            return json!({"error":{"code":"parse.syntax","message":error.to_string()}})
                .to_string();
        }
    };
    HOST.with(|cell| {
        let mut host = cell.borrow_mut();
        host.result = None;
        match host.evaluator.begin(&doc, quality) {
            Ok(()) => json!({"started":true}).to_string(),
            Err(error) => json!({"error":error}).to_string(),
        }
    })
}

/// Runs the document on a fresh evaluator and answers `{"nodes": [...]}` with every node in run order (operator, parameters, inputs, the kind and size of what it made, and what
/// it changed from its main input), or `{"error": ...}` with the node that failed. It neither uses nor disturbs the geometry job in progress, so the page can ask at any time.
#[wasm_bindgen]
#[must_use]
pub fn eval_trace(document: &str, quality: &str) -> String {
    if document.len() > ve_graph::MAX_DOCUMENT_BYTES {
        return json!({"error":{"code":"budget.source","message":"document exceeds 2 MiB"}}).to_string();
    }
    let quality = if quality == "preview" { Quality::Preview } else { Quality::Final };
    let doc = match serde_json::from_str::<ve_graph::Document>(document) {
        Ok(doc) => doc,
        Err(error) => return json!({"error":{"code":"parse.syntax","message":error.to_string()}}).to_string(),
    };
    match Evaluator::default().trace(&doc, quality) {
        Ok(nodes) => json!({"nodes": nodes}).to_string(),
        Err(error) => json!({"error": error}).to_string(),
    }
}

/// Executes one node so the JS worker can process cancellation messages between calls.
#[wasm_bindgen]
#[must_use]
pub fn eval_step() -> String {
    HOST.with(|cell| {
        let mut host = cell.borrow_mut();
        match host.evaluator.step() {
            Ok(Some(result)) => {
                let text = json!({"done":result}).to_string();
                host.result = Some(result);
                text
            }
            Ok(None) => json!({"progress":host.evaluator.evaluated_nodes()}).to_string(),
            Err(error) => json!({"error":error}).to_string(),
        }
    })
}

/// The outputs that have grown since the last call, as they stand now (`{"draft": {...}}`, or `{}` when nothing changed): the shape built up to the
/// node that last fed each output, for the page to show while the rest of the document still evaluates. The `eval_*` readers answer for these parts
/// until the finished scene replaces them. Each part's node is its output's node, so a newer draft replaces an older one.
#[wasm_bindgen]
#[must_use]
pub fn eval_drafts() -> String {
    HOST.with(|cell| {
        let mut host = cell.borrow_mut();
        let parts = host.evaluator.take_drafts();
        if parts.is_empty() {
            return "{}".to_owned();
        }
        let draft = Evaluation {
            parts,
            lights: Vec::new(),
            environment: None,
            scripts: Vec::new(),
            game: None,
            evaluated_nodes: host.evaluator.evaluated_nodes(),
            cache_hits: 0,
            cache_bytes: 0,
            warnings: Vec::new(),
        };
        let text = json!({"draft":draft}).to_string();
        host.result = Some(draft);
        text
    })
}

/// Copies one output's positions into an owned buffer for transfer to the window.
#[wasm_bindgen]
#[must_use]
pub fn eval_positions(part: u32) -> Vec<f32> {
    HOST.with(|cell| {
        cell.borrow()
            .result
            .as_ref()
            .and_then(|r| r.parts.get(part as usize))
            .map_or_else(Vec::new, |p| {
                p.positions.iter().flatten().copied().collect()
            })
    })
}

/// Copies one output's indices into an owned buffer for transfer to the window.
#[wasm_bindgen]
#[must_use]
pub fn eval_indices(part: u32) -> Vec<u32> {
    HOST.with(|cell| {
        cell.borrow()
            .result
            .as_ref()
            .and_then(|r| r.parts.get(part as usize))
            .map_or_else(Vec::new, |p| p.indices.clone())
    })
}

/// Copies authored UV coordinates in triangle-corner order for transfer to the page.
#[wasm_bindgen]
#[must_use]
pub fn eval_uvs(part: u32) -> Vec<f32> {
    HOST.with(|cell| {
        cell.borrow()
            .result
            .as_ref()
            .and_then(|r| r.parts.get(part as usize))
            .map_or_else(Vec::new, |p| p.uvs.iter().flatten().copied().collect())
    })
}

/// Copies one output's baked ambient visibility values into an owned transfer buffer.
#[wasm_bindgen]
#[must_use]
pub fn eval_occlusion(part: u32) -> Vec<f32> {
    HOST.with(|cell| {
        cell.borrow()
            .result
            .as_ref()
            .and_then(|r| r.parts.get(part as usize))
            .map_or_else(Vec::new, |p| p.occlusion.clone())
    })
}

/// Copies one output's explicit vertex normals (three floats per vertex) into an owned transfer buffer; empty when the viewer derives them.
#[wasm_bindgen]
#[must_use]
pub fn eval_normals(part: u32) -> Vec<f32> {
    HOST.with(|cell| {
        cell.borrow()
            .result
            .as_ref()
            .and_then(|r| r.parts.get(part as usize))
            .map_or_else(Vec::new, |p| p.normals.iter().flatten().copied().collect())
    })
}

/// Copies one output's per-vertex colours (four floats per vertex: water flow, depth, foam) into an owned transfer buffer; empty for none.
#[wasm_bindgen]
#[must_use]
pub fn eval_colors(part: u32) -> Vec<f32> {
    HOST.with(|cell| {
        cell.borrow()
            .result
            .as_ref()
            .and_then(|r| r.parts.get(part as usize))
            .map_or_else(Vec::new, |p| p.colors.iter().flatten().copied().collect())
    })
}

/// The side in pixels of the maps `bake.maps` baked onto a part, or 0 when it has none.
#[wasm_bindgen]
#[must_use]
pub fn eval_baked_size(part: u32) -> u32 {
    HOST.with(|cell| {
        cell.borrow()
            .result
            .as_ref()
            .and_then(|r| r.parts.get(part as usize))
            .and_then(|p| p.baked.as_ref())
            .map_or(0, |baked| baked.size)
    })
}

/// A part's baked tangent-space normal map as `size * size * 4` RGBA bytes; empty when it has none.
#[wasm_bindgen]
#[must_use]
pub fn eval_baked_normal(part: u32) -> Vec<u8> {
    HOST.with(|cell| {
        cell.borrow()
            .result
            .as_ref()
            .and_then(|r| r.parts.get(part as usize))
            .and_then(|p| p.baked.as_ref())
            .and_then(|baked| baked.normal.clone())
            .unwrap_or_default()
    })
}

/// A part's baked ambient occlusion as `size * size * 4` RGBA bytes of grey (255 is open); empty when it has none.
#[wasm_bindgen]
#[must_use]
pub fn eval_baked_ao(part: u32) -> Vec<u8> {
    HOST.with(|cell| {
        cell.borrow()
            .result
            .as_ref()
            .and_then(|r| r.parts.get(part as usize))
            .and_then(|p| p.baked.as_ref())
            .and_then(|baked| baked.ao.clone())
            .unwrap_or_default()
    })
}

/// Four bone indices per vertex of a skinned part (ADR 0030); empty for a rigid part.
#[wasm_bindgen]
#[must_use]
pub fn eval_skin_joints(part: u32) -> Vec<u32> {
    HOST.with(|cell| {
        cell.borrow()
            .result
            .as_ref()
            .and_then(|r| r.parts.get(part as usize))
            .and_then(|p| p.skin.as_ref())
            .map_or_else(Vec::new, |s| {
                s.joints.iter().flatten().map(|j| u32::from(*j)).collect()
            })
    })
}

/// Four bone weights per vertex of a skinned part (ADR 0030); empty for a rigid part.
#[wasm_bindgen]
#[must_use]
pub fn eval_skin_weights(part: u32) -> Vec<f32> {
    HOST.with(|cell| {
        cell.borrow()
            .result
            .as_ref()
            .and_then(|r| r.parts.get(part as usize))
            .and_then(|p| p.skin.as_ref())
            .map_or_else(Vec::new, |s| s.weights.iter().flatten().copied().collect())
    })
}

/// Copies instance placements for transferable transport; empty for an ordinary solid.
#[wasm_bindgen]
#[must_use]
pub fn eval_transforms(part: u32) -> Vec<f32> {
    HOST.with(|cell| {
        cell.borrow()
            .result
            .as_ref()
            .and_then(|r| r.parts.get(part as usize))
            .and_then(|p| p.transforms.as_ref())
            .map_or_else(Vec::new, |m| m.iter().flatten().copied().collect())
    })
}

/// Original per-placement indices; reflection batches keep the same authored selection identity.
#[wasm_bindgen]
#[must_use]
pub fn eval_instance_indices(part: u32) -> Vec<u32> {
    HOST.with(|cell| {
        cell.borrow()
            .result
            .as_ref()
            .and_then(|r| r.parts.get(part as usize))
            .map_or_else(Vec::new, |p| p.instance_indices.clone())
    })
}

/// Camera distance band for drawing a placed part, or an empty vector when it has no authored range.
#[wasm_bindgen]
#[must_use]
pub fn eval_visible_range(part: u32) -> Vec<f32> {
    HOST.with(|cell| {
        cell.borrow()
            .result
            .as_ref()
            .and_then(|r| r.parts.get(part as usize))
            .and_then(|p| p.visible_range)
            .map_or_else(Vec::new, |(from, to)| vec![from, to])
    })
}

/// Camera distance cross-fade width for a placed part, or zero when it has none.
#[wasm_bindgen]
#[must_use]
pub fn eval_visible_fade(part: u32) -> f32 {
    HOST.with(|cell| {
        cell.borrow()
            .result
            .as_ref()
            .and_then(|r| r.parts.get(part as usize))
            .map_or(0.0, |p| p.visible_fade)
    })
}

/// Drops pending work/results. Cancellation signals never become cached kernel handles.
#[wasm_bindgen]
pub fn eval_cancel() {
    HOST.with(|cell| {
        let mut host = cell.borrow_mut();
        host.evaluator.cancel();
        host.result = None;
    });
}

/// Retained cache payload and execution counters, for soak evidence.
#[wasm_bindgen]
#[must_use]
pub fn eval_stats() -> String {
    HOST.with(|cell| {
        let host = cell.borrow();
        json!({"cache_bytes":host.evaluator.cache_bytes(),
        "evaluated_nodes":host.evaluator.evaluated_nodes()})
        .to_string()
    })
}

/// Returns the last completed evaluation's game definition and static diagnostics.
#[wasm_bindgen]
#[must_use]
pub fn eval_game() -> String {
    HOST.with(|cell| {
        let host = cell.borrow();
        crate::json::eval_game_json(host.result.as_ref().and_then(|result| result.game.as_ref()))
    })
}

/// Returns the last completed evaluation's scripts (`script.js` nodes) as a JSON array of `{name, source, enabled, node}`.
#[wasm_bindgen]
#[must_use]
pub fn eval_scripts() -> String {
    HOST.with(|cell| {
        let host = cell.borrow();
        host.result
            .as_ref()
            .and_then(|result| serde_json::to_string(&result.scripts).ok())
            .unwrap_or_else(|| "[]".to_owned())
    })
}

/// Measures gaps between evaluated output parts, merging material regions by output name.
#[wasm_bindgen]
#[must_use]
pub fn eval_attachments(tolerance: f32) -> String {
    HOST.with(|cell| {
        let host = cell.borrow();
        let Some(result) = host.result.as_ref() else {
            return json!({"floating":[],"parts":0}).to_string();
        };
        let regions: Vec<_> = result
            .parts
            .iter()
            .map(|part| crate::attachments::Region {
                name: &part.name,
                positions: &part.positions,
                indices: &part.indices,
                transforms: part.transforms.as_deref(),
            })
            .collect();
        crate::attachments::report(&regions, tolerance)
    })
}

/// Parse and run a game definition with a bounded headless policy.
#[wasm_bindgen]
#[must_use]
pub fn game_playtest(text: &str, policy: &str, seed: u32, max_steps: u32) -> String {
    crate::json::game_playtest_json(text, policy, seed, max_steps)
}
