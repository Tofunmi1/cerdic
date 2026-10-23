// The node count below is the real bound on a program's cost; the length only stops pathological text. 600 was too tight for a
// surface with a shared tiling prefix, which a model otherwise pastes into every program.
export const MAX_SOURCE = 2000;
export const MAX_NODES = 160;
export const MAX_IDENTIFIER = 16;
export const MAX_LOOP_BOUND = 64;
export const MAX_LOOP_DEPTH = 2;
export const MAX_LOOP_PRODUCT = 256;
