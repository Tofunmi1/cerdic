//! LRU of exported immutable meshes. No native kernel objects survive in the cache.
use std::collections::BTreeMap;
use std::rc::Rc;
use ve_kernel::Mesh;

pub(crate) struct Value {
    pub(crate) mesh: Mesh,
    pub(crate) volume: f64,
}

impl Value {
    pub(crate) fn bytes(&self) -> usize {
        self.mesh.positions.capacity() * size_of::<[f32; 3]>()
            + self.mesh.indices.capacity() * size_of::<u32>()
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
