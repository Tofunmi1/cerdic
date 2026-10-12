//! The authored document: nodes, links, outputs, materials and the editor layer.
//!
//! Only authored data lives here. Entity IDs, kernel handles, GPU buffers, caches, cameras and hover
//! state belong elsewhere. The canonical JSON form is deterministic: structs serialise in field
//! order and maps are sorted.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::id::{DocId, MaterialId, NodeId};
use crate::spec::OpId;
use crate::value::Value;

/// The document schema this build writes.
pub const SCHEMA: u32 = 1;

/// The only output port every operator has.
pub const OUT_PORT: &str = "out";

/// A reference to an output port of a node.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PortRef {
    /// The node.
    pub node: NodeId,
    /// Its output port (always `out` for now).
    pub port: String,
}

/// Switches on a node.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flags {
    /// Skip the node (its output is empty).
    #[serde(default)]
    pub muted: bool,
    /// Pass the first input through unchanged where the types allow.
    #[serde(default)]
    pub bypass: bool,
}

/// One node of the graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    /// Stable identity.
    pub id: NodeId,
    /// The operator and version.
    pub op: OpId,
    /// A display name; never part of identity or evaluation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Parameter values in canonical form.
    pub params: BTreeMap<String, Value>,
    /// Links by input port name.
    pub inputs: BTreeMap<String, Vec<PortRef>>,
    /// Mute and bypass.
    #[serde(default)]
    pub flags: Flags,
}

/// A named result of the document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Output {
    /// The output's name.
    pub name: String,
    /// The node that provides it.
    pub from: PortRef,
}

/// A material definition (typed in a later gate; free-form data until then).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Material {
    /// Stable identity.
    pub id: MaterialId,
    /// Named values.
    pub def: BTreeMap<String, Value>,
}

/// Where the editor drew things. Saved and undoable, but outside the evaluation hash.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EditorLayer {
    /// Node positions `[x, y]`.
    #[serde(default)]
    pub positions: BTreeMap<NodeId, [f64; 2]>,
    /// Nodes drawn collapsed.
    #[serde(default)]
    pub collapsed: BTreeSet<NodeId>,
}

/// How identifiers are issued. The counter only moves forward, so an ID is never reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdState {
    /// The next value to encode.
    pub next: u64,
    /// Offset that differs between documents.
    pub salt: u64,
}

/// The whole authored document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    /// Schema version of this data.
    pub schema: u32,
    /// Document identity.
    pub id: DocId,
    /// Counts commits, undo and redo; the base of the conflict check.
    pub revision: u64,
    /// Always `m`; present so files say what their numbers mean.
    pub units: String,
    /// Seed for anything random.
    pub seed: u64,
    /// ID issuing state.
    pub ids: IdState,
    /// Nodes in creation order.
    pub nodes: Vec<Node>,
    /// Named results.
    pub outputs: Vec<Output>,
    /// Material definitions.
    pub materials: Vec<Material>,
    /// Editor layer.
    pub editor: EditorLayer,
}

/// A 128-bit non-cryptographic digest used as a cache key component.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EvalHash(pub [u64; 2]);

fn fnv(bytes: &[u8], offset: u64) -> u64 {
    bytes.iter().fold(offset, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01B3)
    })
}

impl Document {
    /// An empty document with a fresh identity derived from `seed`.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        let salt = fnv(&seed.to_le_bytes(), 0xCBF2_9CE4_8422_2325) & ((1 << 40) - 1);
        let mut ids = IdState { next: 0, salt };
        let id = DocId::generated(ids.next, ids.salt);
        ids.next += 1;
        Self {
            schema: SCHEMA,
            id,
            revision: 0,
            units: "m".to_owned(),
            seed,
            ids,
            nodes: Vec::new(),
            outputs: Vec::new(),
            materials: Vec::new(),
            editor: EditorLayer::default(),
        }
    }

    /// Issues a node ID, or `None` if the 40-bit space is used up.
    pub(crate) fn issue_node_id(&mut self) -> Option<NodeId> {
        (self.ids.next < (1 << 40)).then(|| {
            let id = NodeId::generated(self.ids.next, self.ids.salt);
            self.ids.next += 1;
            id
        })
    }

    /// Issues a material ID, or `None` if the 40-bit space is used up.
    pub(crate) fn issue_material_id(&mut self) -> Option<MaterialId> {
        (self.ids.next < (1 << 40)).then(|| {
            let id = MaterialId::generated(self.ids.next, self.ids.salt);
            self.ids.next += 1;
            id
        })
    }

    /// The node with this ID.
    #[must_use]
    pub fn node(&self, id: &NodeId) -> Option<&Node> {
        self.nodes.iter().find(|n| &n.id == id)
    }

    pub(crate) fn node_mut(&mut self, id: &NodeId) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| &n.id == id)
    }

    /// The deterministic JSON form.
    #[must_use]
    pub fn canonical_json(&self) -> String {
        // Serialising plain data structures cannot fail; the empty string would fail every
        // comparison and size check loudly rather than pass silently.
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Whether two documents hold the same authored content, ignoring the revision counter and the
    /// ID issuing state (undo restores content but never rewinds either).
    #[must_use]
    pub fn same_content(&self, other: &Self) -> bool {
        let strip = |d: &Self| Self {
            revision: 0,
            ids: IdState { next: 0, ..d.ids },
            ..d.clone()
        };
        strip(self) == strip(other)
    }

    /// Digest of everything evaluation depends on: operators, parameters, links, flags, outputs,
    /// materials, seed and units. Labels, the editor layer, the revision and ID state are excluded,
    /// so moving or renaming a node never invalidates a cached result.
    #[must_use]
    pub fn eval_hash(&self) -> EvalHash {
        #[derive(Serialize)]
        struct Evaluated<'a> {
            schema: u32,
            units: &'a str,
            seed: u64,
            nodes: Vec<(
                &'a NodeId,
                &'a OpId,
                &'a BTreeMap<String, Value>,
                &'a BTreeMap<String, Vec<PortRef>>,
                Flags,
            )>,
            outputs: &'a [Output],
            materials: &'a [Material],
        }
        let view = Evaluated {
            schema: self.schema,
            units: &self.units,
            seed: self.seed,
            nodes: self
                .nodes
                .iter()
                .map(|n| (&n.id, &n.op, &n.params, &n.inputs, n.flags))
                .collect(),
            outputs: &self.outputs,
            materials: &self.materials,
        };
        let bytes = serde_json::to_vec(&view).unwrap_or_default();
        EvalHash([
            fnv(&bytes, 0xCBF2_9CE4_8422_2325),
            fnv(&bytes, 0x8422_2325_CBF2_9CE4),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_documents_are_deterministic_and_distinct_by_seed() {
        assert_eq!(Document::new(1), Document::new(1));
        assert_ne!(Document::new(1).id, Document::new(2).id);
    }

    #[test]
    fn json_round_trips() {
        let mut doc = Document::new(7);
        let id = doc.issue_node_id().unwrap();
        doc.nodes.push(Node {
            id: id.clone(),
            op: OpId::parse("solid.box@1").unwrap(),
            label: Some("a box".into()),
            params: BTreeMap::from([("size".to_owned(), Value::List(vec![Value::Num(1.0); 3]))]),
            inputs: BTreeMap::new(),
            flags: Flags::default(),
        });
        doc.editor.positions.insert(id, [1.0, 2.0]);
        let json = doc.canonical_json();
        let back: Document = serde_json::from_str(&json).unwrap();
        assert_eq!(back, doc);
        assert_eq!(back.canonical_json(), json);
    }

    #[test]
    fn issued_ids_never_repeat() {
        let mut doc = Document::new(3);
        let first = doc.issue_node_id().unwrap();
        let second = doc.issue_node_id().unwrap();
        let material = doc.issue_material_id().unwrap();
        assert_ne!(first, second);
        assert_ne!(first.as_str()[2..], material.as_str()[2..]);
    }
}
