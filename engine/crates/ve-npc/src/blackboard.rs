use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A typed value stored on an NPC blackboard.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Value {
    /// A boolean value.
    Bool(bool),
    /// A numeric value.
    Number(f64),
    /// A text value.
    Text(String),
    /// A three dimensional point.
    Point([f32; 3]),
}

#[derive(Clone, Debug)]
struct Entry {
    value: Value,
    set_at: f64,
    ttl: Option<f64>,
}

/// Deterministic typed key-value storage with optional expiry.
#[derive(Clone, Default, Debug)]
pub struct Blackboard {
    values: BTreeMap<String, Entry>,
}

impl Blackboard {
    /// Creates an empty blackboard.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Stores a value without expiry.
    pub fn set(&mut self, key: &str, value: Value, now: f64) {
        self.values.insert(
            key.to_owned(),
            Entry {
                value,
                set_at: now,
                ttl: None,
            },
        );
    }
    /// Stores a value that expires when `now > set_time + ttl_secs`.
    pub fn set_for(&mut self, key: &str, value: Value, now: f64, ttl_secs: f64) {
        self.values.insert(
            key.to_owned(),
            Entry {
                value,
                set_at: now,
                ttl: Some(ttl_secs),
            },
        );
    }
    /// Gets a value if it exists and has not expired.
    #[must_use]
    pub fn get(&self, key: &str, now: f64) -> Option<&Value> {
        self.values
            .get(key)
            .filter(|e| !expired(e, now))
            .map(|e| &e.value)
    }
    /// Gets a numeric value.
    #[must_use]
    pub fn get_number(&self, key: &str, now: f64) -> Option<f64> {
        match self.get(key, now)? {
            Value::Number(v) => Some(*v),
            _ => None,
        }
    }
    /// Gets a boolean value.
    #[must_use]
    pub fn get_bool(&self, key: &str, now: f64) -> Option<bool> {
        match self.get(key, now)? {
            Value::Bool(v) => Some(*v),
            _ => None,
        }
    }
    /// Gets a text value.
    #[must_use]
    pub fn get_text(&self, key: &str, now: f64) -> Option<String> {
        match self.get(key, now)? {
            Value::Text(v) => Some(v.clone()),
            _ => None,
        }
    }
    /// Gets a point value.
    #[must_use]
    pub fn get_point(&self, key: &str, now: f64) -> Option<[f32; 3]> {
        match self.get(key, now)? {
            Value::Point(v) => Some(*v),
            _ => None,
        }
    }
    /// Gets elapsed time since the value was set, if it remains valid.
    #[must_use]
    pub fn age(&self, key: &str, now: f64) -> Option<f64> {
        self.values
            .get(key)
            .filter(|e| !expired(e, now))
            .map(|e| now - e.set_at)
    }
    /// Removes a value regardless of expiry.
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        self.values.remove(key).map(|e| e.value)
    }
    /// Returns sorted keys for values that have not expired.
    #[must_use]
    pub fn keys(&self, now: f64) -> Vec<String> {
        self.values
            .iter()
            .filter(|(_, e)| !expired(e, now))
            .map(|(k, _)| k.clone())
            .collect()
    }
}

fn expired(entry: &Entry, now: f64) -> bool {
    entry.ttl.is_some_and(|ttl| now > entry.set_at + ttl)
}
