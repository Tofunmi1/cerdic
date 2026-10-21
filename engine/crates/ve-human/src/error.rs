//! Errors returned by pack loading and body evaluation.
use thiserror::Error;

/// Failure while decoding a pack or evaluating named parameters.
#[derive(Debug, Error)]
pub enum Error {
    /// The compressed data could not be decoded.
    #[error("invalid compressed human pack: {0}")]
    Compression(String),
    /// The pack has an invalid header, section, or index.
    #[error("invalid human pack: {0}")]
    InvalidPack(String),
    /// A requested parameter does not exist.
    #[error("unknown {kind} parameter '{name}'; valid names: {valid}")]
    UnknownParameter {
        /// Parameter family.
        kind: &'static str,
        /// Requested name.
        name: String,
        /// Valid choices.
        valid: String,
    },
    /// A requested parameter is outside its supported interval.
    #[error("{kind} parameter '{name}' is {value}; expected {range}")]
    OutOfRange {
        /// Parameter family.
        kind: &'static str,
        /// Requested name.
        name: String,
        /// Rejected value.
        value: f32,
        /// Accepted interval.
        range: &'static str,
    },
}
