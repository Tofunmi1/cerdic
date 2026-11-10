import type { Document } from './generated';

export type Quality = 'preview' | 'final';
export type Token = {
  document: string; revision: number; request: number; generation: number; quality: Quality; hash: string;
};
export type Candidate = { document: Document; hash: string; pending: boolean };
export type Job = { token: Token; document: Document; test?: 'spin' | 'crash' };
export type PartMaterial = { color: [number, number, number]; opacity?: number; metallic: number; roughness: number; emissive: [number, number, number]; emits_light: boolean; texture_program?: string | null; texture_tile?: number; no_repeat?: boolean; no_repeat_scale?: number; texture_size?: number; texture_style?: string | null; toon?: { bands: number; feather: number; shift: number; spec: number; shade_color: [number, number, number]; rim: number; outline?: number; outline_color?: [number, number, number] } | null; foliage?: { alpha_cutoff: number; sway: number; flutter: number; grass?: boolean; ground_color?: [number, number, number]; ground_blend?: number; fade_near?: number; fade_far?: number; translucency?: number; exposure_boost?: number } | null };
export type SceneLight = { node: string; name: string; kind: 'point' | 'spot' | 'sun'; position: [number, number, number]; direction: [number, number, number]; color: [number, number, number]; intensity: number; range: number; inner_angle: number; outer_angle: number };
export type SceneEnvironment = {
  node: string; sky_color: [number, number, number]; ground_color: [number, number, number];
  intensity: number; reflection: number; bounce_color: [number, number, number]; bounce: number;
};
/** One keyframed transform track of a part (from an `anim.track` node): metres, degrees or scale factors per axis. */
export type AnimationTrack = {
  property: 'position' | 'rotation' | 'scale' | 'glow' | 'visible' | 'color';
  keys: { time: number; value: [number, number, number] }[];
  interpolation: 'linear' | 'smooth' | 'ease_in' | 'ease_out' | 'bezier' | 'back' | 'bounce' | 'elastic' | 'step';
  looped: boolean;
  pivot: [number, number, number];
  /** The named clip the track belongs to; empty or absent is the part's main clip. */
  clip?: string;
  /** For a placed set: seconds each copy plays ahead per index, and up to this many more by a fixed pseudo-random offset. */
  phase_step?: number; phase_jitter?: number;
};
export type MorphTarget = { name: string; weight: number; positions: Float32Array };
export type MeshPart = {
  node: string; name: string; volume: number; positions: Float32Array; indices: Uint32Array; occlusion: Float32Array; material?: PartMaterial | null;
  /** Optional per-triangle-corner UVs, two floats per index. Missing values use generated box charts on export. */
  uvs?: Float32Array;
  /** Optional explicit unit normals, three floats per vertex (a terrain tile, whose edge normals depend on its neighbours). Empty or missing: derived from the mesh. */
  normals?: Float32Array;
  /** Optional per-vertex colours, four floats per vertex (water: flow x and z remapped to 0..1, depth, foam). Empty or missing: none. */
  colors?: Float32Array;
  /** Evaluator-baked tangent-space normal and ambient occlusion maps. */
  baked?: { size: number; normal?: Uint8Array; ao?: Uint8Array };
  /** Which colour of a fused multi-colour solid this part draws (0 for ordinary solids). */
  region?: number;
  transforms?: Float32Array; instance_indices?: Uint32Array;
  /** Camera distance band for drawing placed copies, in metres. */
  visible_range?: [number, number];
  /** Cross-fade width at each finite edge of the camera distance band, in metres. */
  visible_fade?: number;
  frames?: Record<string,{position:[number,number,number];rotation:[number,number,number,number]}>;
  /** Keyframed tracks that move the part; absent or empty for a part that stays put. */
  animation?: AnimationTrack[];
  /** The animation state machine on the part (from an `anim.machine` node): which clips play, and when; absent or null for none. */
  state_machine?: unknown;
  /** What starts the part's clip in play mode (from a `behavior.on` node); absent or null for a part that does not react. */
  /** A bone part's rest segment (from `rig.bone`), in world coordinates. */
  bone?: { head: [number, number, number]; tail: [number, number, number] } | null;
  /** A skinned part's bones (from `rig.skin`, after the weights were computed); null for a rigid part. */
  skin?: { bones: string[]; falloff: number; influences: number; blend?: 'linear' | 'dual_quaternion'; paint?: unknown[] } | null;
  /** What the part is as a physics body (from `phys.body`); absent or null for a part that is not simulated. */
  body?: { kind: 'static' | 'dynamic' | 'kinematic'; shape: 'auto' | 'box' | 'sphere' | 'capsule' | 'cylinder' | 'convex' | 'mesh';
    density: number; friction: number; restitution: number; linear_damping: number; angular_damping: number } | null;
  /** Four bone indices per vertex (into `skin.bones`) and four weights per vertex, for a skinned part. */
  skin_joints?: Uint32Array; skin_weights?: Float32Array;
  /** Named per-vertex position deltas for shape keys, with initial weights for glTF export. */
  morph_targets?: MorphTarget[];
  /** For a part that is the target of a chain of bones (from `rig.ik`): the bones, root first, and their rest segments. */
  ik?: { bones: string[]; iterations: number; segments: { head: [number, number, number]; tail: [number, number, number] }[] } | null;
  /** For a part that carries a ragdoll definition (from `phys.ragdoll`, resolved against its bones). */
  ragdoll?: { name: string; bones: string[]; segments: unknown[]; parents: (number | null)[]; hinges: boolean[] } | null;
  /** For a part that is live cloth (from `cloth.sim`): pins, the bones it follows and collides with, and its physical settings. */
  cloth?: { pins: number[]; pin_bone: string; collide_bones: string[]; segments: unknown[] } & Record<string, unknown> | null;
  /** For a part that is a live liquid (from `fluid.sim`): the block, container, thickness and collider bones. */
  fluid?: { min: number[]; max: number[]; spacing: number; collide_bones: string[]; segments: unknown[]; max_triangles: number } & Record<string, unknown> | null;
  /** For a part drawn by a simulation program (from `sim.program`): the SimL source, seed, rate, particle size and the drawn state array. */
  sim?: { source: string; seed: number; rate: number; size: number; particles: string; count: number; mode: string; bones: string[]; values: [string, number[]][] } | null;
  /** For a part that emits particles (from `fx.emitter`): the effect (emitters and their settings), the emitters' names and the ground height. */
  fx?: { effect: { emitters: unknown[] }; names: string[]; ground: number | null } | null;
  /** The output name of the part this one follows (from `anim.parent`); absent or null for a free part. */
  parent?: string | null;
  behavior?: {
    trigger: 'select' | 'near' | 'signal' | 'flag' | 'grab' | 'release' | 'hold' | 'timer' | 'socket'; target: string | null; radius: number; action: 'play' | 'toggle' | 'follow';
    sets: { name: string; change: 'on' | 'off' | 'toggle' }[]; needs: { name: string; value: boolean }[]; watch: string | null;
    after: number; every: number; accepts: string | null; clip: string; target_clip: string; then: string[];
  } | null;
};

/** Reads evaluator baked-map exports, omitting the field for an unbaked part. */
export function bakedMapsFromEvaluator(size: number, normal: () => Uint8Array, ao: () => Uint8Array): MeshPart['baked'] {
  return size > 0 ? { size, normal: normal(), ao: ao() } : undefined;
}
export type EvaluationStats = {
  evaluated_nodes: number; cache_hits: number; cache_bytes: number; elapsed_ms: number; memory_bytes: number;
};
export type EvaluationWarning = { code: string; node: string; message: string };
/** The game the last evaluation carried (ADR 0062): its text form, a one-line summary and what `GameDef::validate` found. */
export type GameInfo = { present: boolean; text?: string; summary?: string; diagnostics?: Array<{ code: string; message: string; suggestion?: string }> };
export type Attachments = { floating: { name: string; gap: number; nearest: string }[]; parts: number; error?: string };
export type WorkerMessage =
  | { kind: 'ready'; startup_ms: number }
  | { kind: 'cancelled'; token: Token }
  | { kind: 'progress'; token: Token; evaluated: number; total: number }
  | { kind: 'draft'; token: Token; parts: MeshPart[]; evaluated: number; total: number }
  | { kind: 'playtest_result'; id: number; report: string }
  | { kind: 'document_text_result'; id: number; reply: import('./evaluator-text').TextReply }
  | { kind: 'result'; token: Token; parts: MeshPart[]; lights: SceneLight[]; environment?: SceneEnvironment | null; warnings: EvaluationWarning[]; game?: GameInfo; scripts?: Array<{ name: string; source: string; enabled: boolean; node: string }>; attachments?: Attachments; bytes: number; sent_ms: number; stats: EvaluationStats }
  | { kind: 'failure'; token: Token; error: { code: string; message: string } }
  | { kind: 'crash'; token: Token; error: { code: string; message: string } };

export function sameToken(a: Token | null, b: Token): boolean {
  return !!a && a.document === b.document && a.revision === b.revision && a.request === b.request
    && a.generation === b.generation && a.quality === b.quality && a.hash === b.hash;
}
