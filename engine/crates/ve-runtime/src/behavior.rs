//! Play mode and part behaviours (`behavior.on`, ADR 0027): in play mode a click or the viewer coming near
//! starts a part's own clip. Behaviours run at a fixed step, so the same triggers on the same step numbers
//! give the same clip times everywhere.

include!("behavior/part_1.rs");
include!("behavior/part_2.rs");
