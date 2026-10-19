//! Pure pointer-ray constraints for document-editing gizmos.
//!
//! A host owns the drag lifetime and commits the returned delta through its typed command system.
//! These calculations never move a rendered entity or mutate an authored document.

mod drag;
mod view;

pub use drag::{Constraint, Delta, GizmoDrag, Mode, Snap};
pub use view::CameraView;
