//! Operator descriptions: what a node accepts, produces and promises.
//!
//! Help text, provider schemas, inspector widgets and node-search entries are generated from these.
//! Operators live in the shared `ve_core::Registry`, keyed `family.name.vN` so several versions of
//! one operator can coexist; a node refers to one by `family.name@N`.

use std::fmt;

use serde::{Deserialize, Serialize};
use ve_core::{Name, Registry, RegistryError};

use crate::value::{ParamType, Value};

/// A kind of value that flows along a link.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PortType {
    /// A closed solid.
    Solid,
    /// A planar profile.
    Profile,
    /// A space curve.
    Curve,
    /// An implicit surface (signed distance field).
    Sdf,
    /// A set of instances.
    Inst,
    /// The verdict of a constraint node; carries no geometry.
    Check,
}

/// How many links an input port takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Arity {
    /// Exactly one link (a new link replaces the old one).
    One,
    /// A list of links, at least `min` for a complete graph.
    Variadic {
        /// Fewest links for a complete graph.
        min: u8,
    },
}

/// An input port.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PortSpec {
    /// The port's name.
    pub name: &'static str,
    /// The kinds of value it accepts.
    pub accepts: &'static [PortType],
    /// How many links it takes.
    pub arity: Arity,
}

/// What an operator produces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "port", rename_all = "snake_case")]
pub enum OutputSpec {
    /// Always this type.
    Fixed(PortType),
    /// The type arriving on the first link of the named input port.
    SameAsInput(&'static str),
}

/// A parameter of an operator.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ParamSpec {
    /// The parameter's name.
    pub name: &'static str,
    /// Its type and bounds.
    pub ty: ParamType,
    /// The value used when none is given; `None` makes the parameter required.
    pub default: Option<Value>,
    /// One line of help.
    pub doc: &'static str,
}

/// Whether an operator may be offered to providers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Readiness {
    /// Can be stored in documents; not evaluated or offered to providers yet.
    Draft,
    /// Its conformance cases pass; offered in schemas.
    Ready,
}

/// Identifies one version of one operator, written `family.name@N`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS), ts(type = "string"))]
pub struct OpId {
    family: Name,
    version: u32,
}

/// Why an operator ID could not be read.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{0:?} is not an operator id like \"solid.box@1\"")]
pub struct OpIdError(pub String);

impl OpId {
    /// Builds an ID from a family name and a version of at least 1.
    ///
    /// # Errors
    /// [`OpIdError`] if the family is not a valid [`Name`] or the version is zero.
    pub fn new(family: &str, version: u32) -> Result<Self, OpIdError> {
        let name = Name::new(family).map_err(|_| OpIdError(family.to_owned()))?;
        if version == 0 {
            return Err(OpIdError(format!("{family}@0")));
        }
        Ok(Self {
            family: name,
            version,
        })
    }

    /// Reads `family.name@N`.
    ///
    /// # Errors
    /// [`OpIdError`] if the text is not in that form.
    pub fn parse(text: &str) -> Result<Self, OpIdError> {
        let (family, version) = text
            .split_once('@')
            .ok_or_else(|| OpIdError(text.to_owned()))?;
        let version = version
            .parse::<u32>()
            .map_err(|_| OpIdError(text.to_owned()))?;
        Self::new(family, version).map_err(|_| OpIdError(text.to_owned()))
    }

    /// The family name, such as `solid.box`.
    #[must_use]
    pub fn family(&self) -> &Name {
        &self.family
    }

    /// The version number.
    #[must_use]
    pub fn version(&self) -> u32 {
        self.version
    }

    /// A copy with another version.
    ///
    /// # Errors
    /// [`OpIdError`] if `version` is zero.
    pub fn with_version(&self, version: u32) -> Result<Self, OpIdError> {
        Self::new(self.family.as_str(), version)
    }

    fn registry_key(&self) -> Option<Name> {
        Name::new(&format!("{}.v{}", self.family, self.version)).ok()
    }
}

impl TryFrom<String> for OpId {
    type Error = OpIdError;
    fn try_from(s: String) -> Result<Self, OpIdError> {
        Self::parse(&s)
    }
}

impl From<OpId> for String {
    fn from(id: OpId) -> String {
        id.to_string()
    }
}

impl fmt::Display for OpId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.family, self.version)
    }
}

/// The full description of an operator.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct OperatorSpec {
    /// Which operator and version this describes.
    pub id: OpId,
    /// One line of help.
    pub doc: &'static str,
    /// Input ports, in order.
    pub inputs: Vec<PortSpec>,
    /// What it produces.
    pub output: OutputSpec,
    /// Parameters, in order.
    pub params: Vec<ParamSpec>,
    /// Whether providers may use it.
    pub readiness: Readiness,
}

impl OperatorSpec {
    /// The input port called `name`.
    #[must_use]
    pub fn input(&self, name: &str) -> Option<&PortSpec> {
        self.inputs.iter().find(|p| p.name == name)
    }

    /// The parameter called `name`.
    #[must_use]
    pub fn param(&self, name: &str) -> Option<&ParamSpec> {
        self.params.iter().find(|p| p.name == name)
    }
}

/// All known operators, ordered by name and version.
#[derive(Clone, Debug, Default)]
pub struct OperatorRegistry {
    entries: Registry<OperatorSpec>,
}

impl OperatorRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an operator version.
    ///
    /// # Errors
    /// [`RegistryError::Duplicate`] if that version is already registered.
    pub fn register(&mut self, spec: OperatorSpec) -> Result<(), RegistryError> {
        let key = spec
            .id
            .registry_key()
            .ok_or_else(|| RegistryError::Duplicate(spec.id.family().clone()))?;
        self.entries.register(key, spec)
    }

    /// Looks up one operator version.
    #[must_use]
    pub fn get(&self, id: &OpId) -> Option<&OperatorSpec> {
        self.entries.get(&id.registry_key()?)
    }

    /// Every operator version, ordered by name.
    pub fn iter(&self) -> impl Iterator<Item = &OperatorSpec> {
        self.entries.iter().map(|(_, spec)| spec)
    }

    /// Only the operators providers may use.
    pub fn ready(&self) -> impl Iterator<Item = &OperatorSpec> {
        self.iter().filter(|s| s.readiness == Readiness::Ready)
    }

    /// Operator ids closest to `wanted` by edit distance, for "did you mean" hints.
    #[must_use]
    pub fn suggest(&self, wanted: &str) -> Vec<String> {
        let mut scored: Vec<(usize, String)> = self
            .iter()
            .map(|s| s.id.to_string())
            .map(|id| (distance(wanted, &id), id))
            .filter(|(d, _)| *d <= 3)
            .collect();
        scored.sort();
        scored.into_iter().take(3).map(|(_, id)| id).collect()
    }
}

fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row.first().copied().unwrap_or(0);
        if let Some(first) = row.first_mut() {
            *first = i + 1;
        }
        for (j, cb) in b.iter().enumerate() {
            let above = row.get(j + 1).copied().unwrap_or(0);
            let left = row.get(j).copied().unwrap_or(0);
            let cost = usize::from(ca != *cb);
            if let Some(cell) = row.get_mut(j + 1) {
                *cell = (above + 1).min(left + 1).min(diagonal + cost);
            }
            diagonal = above;
        }
    }
    row.last().copied().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operator_ids_round_trip_and_reject_garbage() {
        let id = OpId::parse("solid.box@1").unwrap();
        assert_eq!((id.family().as_str(), id.version()), ("solid.box", 1));
        assert_eq!(id.to_string(), "solid.box@1");
        for bad in [
            "",
            "solid.box",
            "solid.box@",
            "solid.box@0",
            "Solid.Box@1",
            "solid..box@1",
            "@1",
            "a@b",
        ] {
            assert!(OpId::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn suggestions_prefer_close_names() {
        let registry = crate::builtin::builtin_registry();
        let hints = registry.suggest("solid.revolv@1");
        assert_eq!(hints.first().map(String::as_str), Some("solid.revolve@1"));
        assert!(registry.suggest("zzzzzzzzzzzzzz").is_empty());
    }
}
