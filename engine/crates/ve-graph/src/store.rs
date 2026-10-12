//! Saving and loading documents.
//!
//! A save is a text envelope (format, schema, generation, checksum, operator manifest, document).
//! Browsers cannot rename files atomically, so a document lives in two slots: a save always goes to
//! the slot that does *not* hold the newest intact save, and a load takes the newest intact slot.
//! A torn or failed write can therefore only damage the older slot; the newest save survives.
//! Loading never writes, and saving refuses to overwrite a file from a newer version.

mod envelope;
mod memory;
mod migrate;

use crate::diag::{Diagnostic, Severity};
use crate::doc::Document;
use crate::limits::{MAX_DOCUMENT_BYTES, count};
use crate::spec::OperatorRegistry;

pub use memory::MemoryBackend;
pub use migrate::{MigrationStep, Migrator};

use envelope::Peek;

/// Why a storage operation failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackendError {
    /// The store is full; `needed` is the size of the value that did not fit.
    QuotaExceeded {
        /// Bytes the refused value needed.
        needed: usize,
    },
    /// Any other failure (storage disabled, blocked by privacy settings, ...).
    Failed(String),
}

/// A key-value text store (browser local storage, or memory in tests).
pub trait Backend {
    /// The value under `key`, if any.
    ///
    /// # Errors
    /// [`BackendError::Failed`] if the store cannot be read.
    fn get(&self, key: &str) -> Result<Option<String>, BackendError>;

    /// Stores `value` under `key`, replacing any previous value atomically.
    ///
    /// # Errors
    /// [`BackendError::QuotaExceeded`] if it does not fit, [`BackendError::Failed`] otherwise.
    fn set(&mut self, key: &str, value: &str) -> Result<(), BackendError>;
}

/// A document read from storage.
#[derive(Debug)]
pub struct Loaded {
    /// The document, migrated to the current schema.
    pub document: Document,
    /// Which save it was (0 for files from before saves were numbered).
    pub generation: u64,
    /// Problems found that did not stop the load (unknown operators, a damaged older slot).
    pub issues: Vec<Diagnostic>,
    /// The application must not save over this document (it holds data this build cannot edit).
    pub read_only: bool,
    /// The schema the file was migrated from, if it was older than the current one.
    pub migrated_from: Option<u32>,
}

/// One document's two slots in a [`Backend`].
pub struct DocStore<B> {
    backend: B,
    name: String,
}

impl<B: Backend> DocStore<B> {
    /// A store for the document called `name` (letters, digits, `_` and `-` are safe).
    #[must_use]
    pub fn new(backend: B, name: &str) -> Self {
        Self {
            backend,
            name: name.to_owned(),
        }
    }

    /// The backend, for inspection.
    #[must_use]
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// The backend, for tests that damage it.
    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    fn key(&self, slot: char) -> String {
        format!("ve.doc.{}.{slot}", self.name)
    }

    fn read(&self, slot: char) -> Result<Option<String>, Box<Diagnostic>> {
        self.backend.get(&self.key(slot)).map_err(|e| {
            Box::new(Diagnostic::error(
                "store.unavailable",
                format!("cannot read storage: {e:?}"),
            ))
        })
    }

    /// Saves `doc`, returning the new generation. The previous save stays intact until a later save
    /// replaces its slot.
    ///
    /// # Errors
    /// `budget.source` for an oversized document, `store.newer_version` if a stored file is from a
    /// newer version (it is never overwritten), `store.quota` if it does not fit, and
    /// `store.unavailable` if storage fails. The previous save is intact in every case.
    pub fn save(&mut self, doc: &Document) -> Result<u64, Box<Diagnostic>> {
        let size = doc.canonical_json().len();
        if size > MAX_DOCUMENT_BYTES {
            return Err(Box::new(
                Diagnostic::error("budget.source", "the document is too large to save")
                    .with_limit(count(MAX_DOCUMENT_BYTES), count(size)),
            ));
        }
        let mut newest: Option<(char, u64)> = None;
        for slot in ['a', 'b'] {
            let Some(text) = self.read(slot)? else {
                continue;
            };
            match envelope::peek(&text) {
                Peek::Newer => {
                    return Err(Box::new(
                        Diagnostic::error(
                            "store.newer_version",
                            "a saved file is from a newer version; saving would overwrite it",
                        )
                        .suggest("open it with a newer build"),
                    ));
                }
                Peek::Valid { generation, .. } => {
                    if newest.is_none_or(|(_, g)| generation > g) {
                        newest = Some((slot, generation));
                    }
                }
                Peek::Invalid => {}
            }
        }
        let (target, generation) = match newest {
            Some(('a', g)) => ('b', g + 1),
            Some((_, g)) => ('a', g + 1),
            None => ('a', 1),
        };
        let text = envelope::encode(doc, generation)?;
        self.backend
            .set(&self.key(target), &text)
            .map_err(|e| match e {
                BackendError::QuotaExceeded { needed } => Box::new(
                    Diagnostic::error("store.quota", "storage is full; the previous save was kept")
                        .with_limit(count(needed), count(needed)),
                ),
                BackendError::Failed(why) => Box::new(Diagnostic::error(
                    "store.unavailable",
                    format!("cannot write storage: {why}; the previous save was kept"),
                )),
            })?;
        Ok(generation)
    }

    /// Loads the newest intact save, or `None` if nothing was ever saved.
    ///
    /// # Errors
    /// `store.corrupt` if every stored slot is damaged, `store.newer_version` if a stored file is
    /// from a newer version, `store.migration_failed` if a migration step fails, or a budget error
    /// for an oversized file. Storage is never modified.
    pub fn load(
        &self,
        registry: &OperatorRegistry,
        migrator: &Migrator,
    ) -> Result<Option<Loaded>, Box<Diagnostic>> {
        let mut best: Option<Loaded> = None;
        let mut failures: Vec<Box<Diagnostic>> = Vec::new();
        for slot in ['a', 'b'] {
            let Some(text) = self.read(slot)? else {
                continue;
            };
            match envelope::decode(&text, migrator, registry) {
                Ok(loaded)
                    if best
                        .as_ref()
                        .is_none_or(|b| loaded.generation > b.generation) =>
                {
                    best = Some(loaded);
                }
                Ok(_) => {}
                Err(d) if d.code == "store.newer_version" => return Err(d),
                Err(d) => failures.push(d),
            }
        }
        match (best, failures.into_iter().next()) {
            (Some(mut loaded), Some(damaged)) => {
                let mut warning = *damaged;
                warning.severity = Severity::Warning;
                warning.message = format!(
                    "one saved copy was damaged and ignored: {}",
                    warning.message
                );
                loaded.issues.push(warning);
                Ok(Some(loaded))
            }
            (Some(loaded), None) => Ok(Some(loaded)),
            (None, Some(damaged)) => Err(damaged),
            (None, None) => Ok(None),
        }
    }
}
