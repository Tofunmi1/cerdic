//! Interaction in the running app: objects marked [`Interactable`] can be hovered, clicked, grabbed
//! and (with two hands) scaled and turned, by mouse or by controllers, through one state machine
//! (`ve-interact`). This module is the glue: it gathers pointers, lets the machine run, applies the
//! placements it returns, tints what is hovered and draws each controller's laser.

include!("interact/part_1.rs");
include!("interact/part_2.rs");
