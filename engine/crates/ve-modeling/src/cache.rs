//! LRU of exported immutable meshes. No native kernel objects survive in the cache.
use crate::Material;
use std::collections::BTreeMap;
use std::rc::Rc;
use ve_kernel::Mesh;

/// What one node produced: a solid's mesh, a profile's canonical f64 loops, or neither (a check).
pub(crate) struct Value {
    pub(crate) mesh: Mesh,
    pub(crate) volume: f64,
    pub(crate) loops: Vec<Vec<[f64; 2]>>,
    /// The surface a `solid.material` gave this solid; transforms and booleans keep their first
    /// input's, everything else starts without one.
    pub(crate) material: Option<Material>,
}

impl Value {
    pub(crate) fn solid(mesh: Mesh, volume: f64) -> Self {
        Self {
            mesh,
            volume,
            loops: Vec::new(),
            material: None,
        }
    }

    pub(crate) fn profile(loops: Vec<Vec<[f64; 2]>>) -> Self {
        Self {
            mesh: Mesh::default(),
            volume: 0.0,
            loops,
            material: None,
        }
    }

    pub(crate) fn nothing() -> Self {
        Self::solid(Mesh::default(), 0.0)
    }

    pub(crate) fn bytes(&self) -> usize {
        self.mesh.positions.capacity() * size_of::<[f32; 3]>()
            + self.mesh.indices.capacity() * size_of::<u32>()
            + self
                .loops
                .iter()
                .map(|l| l.capacity() * size_of::<[f64; 2]>() + size_of::<Vec<[f64; 2]>>())
                .sum::<usize>()
            + size_of::<Self>()
    }
}

struct Entry {
    value: Rc<Value>,
    tick: u64,
    bytes: usize,
}

pub(crate) struct Cache {
    entries: BTreeMap<String, Entry>,
    bytes: usize,
    cap: usize,
    tick: u64,
}

impl Cache {
    pub(crate) fn new(cap: usize) -> Self {
        Self {
            entries: BTreeMap::new(),
            bytes: 0,
            cap,
            tick: 0,
        }
    }
    pub(crate) fn bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn get(&mut self, key: &str) -> Option<Rc<Value>> {
        self.tick += 1;
        let entry = self.entries.get_mut(key)?;
        entry.tick = self.tick;
        Some(Rc::clone(&entry.value))
    }
    pub(crate) fn insert(&mut self, key: String, value: Rc<Value>) {
        let bytes = key.capacity() + value.bytes() + size_of::<Entry>() + 128;
        if bytes > self.cap {
            return;
        }
        if let Some(old) = self.entries.remove(&key) {
            self.bytes -= old.bytes;
        }
        while self.bytes.saturating_add(bytes) > self.cap {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.tick)
                .map(|(key, _)| key.clone());
            let Some(oldest) = oldest else {
                break;
            };
            if let Some(old) = self.entries.remove(&oldest) {
                self.bytes -= old.bytes;
            }
        }
        self.tick += 1;
        self.bytes += bytes;
        self.entries.insert(
            key,
            Entry {
                value,
                tick: self.tick,
                bytes,
            },
        );
    }
}
