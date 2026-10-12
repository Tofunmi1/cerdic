//! The text form of a document: one node per line, the same typed commands underneath.
//!
//! ```text
//! outer = profile.polygon@1(points=[[0, 0], [0.06, 0], [0.05, 0.3]])
//! body  = solid.revolve@1(outer, angle=360deg, segments=64)
//! output vase = body
//! ```
//! Inputs are written by node name (positional in port order, or `port=name`), parameters as
//! `key=value`. Lengths are metres (`mm` and `cm` suffixes allowed); angles are degrees unless
//! written with `rad`, and canonical text always uses `rad`. Only `ready` operators are accepted.

mod build;
mod coerce;
mod lex;
mod node;
mod parse;
mod print;

pub use build::{TextBatch, parse_text, parse_text_in};
pub use print::to_text;
