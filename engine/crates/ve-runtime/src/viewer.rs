//! The desktop viewer: look around any scene with the mouse and keyboard, with an adjustable (wide)
//! field of view and an optional split-screen mock headset. It is for seeing the engine's work on a
//! laptop without a headset. The page owns the DOM side (pointer lock, keys) and feeds it in through
//! the functions below; nothing here runs unless the page enables it, so scenes stay exactly as
//! authored otherwise.

include!("viewer/part_1.rs");
include!("viewer/part_2.rs");
