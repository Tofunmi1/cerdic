//! The point maps behind the solid deformers: each moves one point given the solid's bounding box.
//!
//! All maps are smooth and use only `+ - * /`, `sin`/`cos` and integer hashing, so the refinement, the orientation check
//! and the final displacement agree and native and WASM results are the same.

include!("maps/part_1.rs");
include!("maps/part_2.rs");
