//! The extension registry.

use std::collections::BTreeMap;

use crate::Name;

/// A named collection of extensions of one kind (for example geometry nodes).
///
/// Iteration order is by name, so anything generated from a registry (LLM tool schemas, docs,
/// snapshots) is deterministic and diff-friendly.
#[derive(Clone, Debug)]
pub struct Registry<T> {
    entries: BTreeMap<Name, T>,
}

/// Why a registration failed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    /// The name is already taken. Re-registering would silently change behaviour.
    #[error("{0} is already registered")]
    Duplicate(Name),
}

impl<T> Default for Registry<T> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }
}

impl<T> Registry<T> {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `value` under `name`.
    ///
    /// # Errors
    /// [`RegistryError::Duplicate`] if `name` is already registered; the registry is unchanged.
    pub fn register(&mut self, name: Name, value: T) -> Result<(), RegistryError> {
        if self.entries.contains_key(&name) {
            return Err(RegistryError::Duplicate(name));
        }
        self.entries.insert(name, value);
        Ok(())
    }

    /// Looks up an extension by name.
    #[must_use]
    pub fn get(&self, name: &Name) -> Option<&T> {
        self.entries.get(name)
    }

    /// All entries, ordered by name.
    pub fn iter(&self) -> impl Iterator<Item = (&Name, &T)> {
        self.entries.iter()
    }

    /// Number of registered extensions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(s: &str) -> Name {
        Name::new(s).unwrap()
    }

    #[test]
    fn rejects_duplicates_and_keeps_first() {
        let mut r = Registry::new();
        r.register(n("box"), 1).unwrap();
        assert_eq!(
            r.register(n("box"), 2),
            Err(RegistryError::Duplicate(n("box")))
        );
        assert_eq!(r.get(&n("box")), Some(&1));
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn iterates_in_name_order() {
        let mut r = Registry::new();
        for name in ["union", "box", "sphere"] {
            r.register(n(name), ()).unwrap();
        }
        let order: Vec<_> = r.iter().map(|(k, ())| k.as_str()).collect();
        assert_eq!(order, ["box", "sphere", "union"]);
    }
}
