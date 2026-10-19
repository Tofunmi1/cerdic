//! Small worker module; the rendering module never needs to run modeling operations.
#![cfg(target_arch = "wasm32")]

use serde_json::json;
use std::cell::RefCell;
use ve_modeling::{Evaluation, Evaluator, Quality};
use wasm_bindgen::prelude::*;

#[derive(Default)]
struct Host {
    evaluator: Evaluator,
    result: Option<Evaluation>,
}
thread_local! { static HOST: RefCell<Host> = RefCell::new(Host::default()); }

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
