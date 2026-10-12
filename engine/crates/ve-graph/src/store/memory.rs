//! An in-memory [`Backend`] that can simulate a full store and a torn write.

use std::collections::BTreeMap;

use crate::store::{Backend, BackendError};

/// Text storage in memory. By default it never fails.
#[derive(Debug, Default)]
pub struct MemoryBackend {
    entries: BTreeMap<String, String>,
    quota: Option<usize>,
    torn_after: Option<usize>,
    writes: usize,
}

impl MemoryBackend {
    /// An empty store with no limits.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Refuses any value larger than `bytes` with [`BackendError::QuotaExceeded`].
    #[must_use]
    pub fn with_quota(mut self, bytes: usize) -> Self {
        self.quota = Some(bytes);
        self
    }

    /// Makes the write after `writes` successful ones store only the first half of its value and
    /// report success, as a crash in the middle of a write would leave it.
    #[must_use]
    pub fn with_torn_write_after(mut self, writes: usize) -> Self {
        self.torn_after = Some(writes);
        self
    }

    /// The raw value under `key`.
    #[must_use]
    pub fn raw(&self, key: &str) -> Option<&String> {
        self.entries.get(key)
    }

    /// Replaces the raw value under `key` (tests use this to damage a file).
    pub fn put_raw(&mut self, key: &str, value: &str) {
        self.entries.insert(key.to_owned(), value.to_owned());
    }
}

impl Backend for MemoryBackend {
    fn get(&self, key: &str) -> Result<Option<String>, BackendError> {
        Ok(self.entries.get(key).cloned())
    }

    fn set(&mut self, key: &str, value: &str) -> Result<(), BackendError> {
        if self.quota.is_some_and(|q| value.len() > q) {
            return Err(BackendError::QuotaExceeded {
                needed: value.len(),
            });
        }
        let stored = if self.torn_after == Some(self.writes) {
            let mut cut = value.len() / 2;
            while !value.is_char_boundary(cut) {
                cut -= 1;
            }
            &value[..cut]
        } else {
            value
        };
        self.writes += 1;
        self.entries.insert(key.to_owned(), stored.to_owned());
        Ok(())
    }
}
