import type { Document } from './generated';

export type Quality = 'preview' | 'final';
export type Token = {
  document: string; revision: number; request: number; generation: number; quality: Quality; hash: string;
};
export type Candidate = { document: Document; hash: string; pending: boolean };
export type Job = { token: Token; document: Document; test?: 'spin' | 'crash' };
export type PartMaterial = { color: [number, number, number]; opacity?: number; metallic: number; roughness: number; emissive: [number, number, number]; emits_light: boolean; texture_program?: string | null; texture_tile?: number; texture_size?: number; texture_style?: string | null; toon?: { bands: number; feather: number; shift: number; spec: number; shade_color: [number, number, number]; rim: number; outline?: number; outline_color?: [number, number, number] } | null; foliage?: { alpha_cutoff: number; sway: number; flutter: number } | null };
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
export type MeshPart = {
  node: string; name: string; volume: number; positions: Float32Array; indices: Uint32Array; occlusion: Float32Array; material?: PartMaterial | null;
  /** Optional per-triangle-corner UVs, two floats per index. Missing values use generated box charts on export. */
  uvs?: Float32Array;
  /** Evaluator-baked tangent-space normal and ambient occlusion maps. */
  baked?: { size: number; normal?: Uint8Array; ao?: Uint8Array };
  /** Which colour of a fused multi-colour solid this part draws (0 for ordinary solids). */
  region?: number;
  transforms?: Float32Array; instance_indices?: Uint32Array;
  frames?: Record<string,{position:[number,number,number];rotation:[number,number,number,number]}>;
  /** Keyframed tracks that move the part; absent or empty for a part that stays put. */
  animation?: AnimationTrack[];
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
  /** For a part that is the target of a chain of bones (from `rig.ik`): the bones, root first, and their rest segments. */
  ik?: { bones: string[]; iterations: number; segments: { head: [number, number, number]; tail: [number, number, number] }[] } | null;
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
export type WorkerMessage =
  | { kind: 'ready'; startup_ms: number }
  | { kind: 'cancelled'; token: Token }
  | { kind: 'result'; token: Token; parts: MeshPart[]; lights: SceneLight[]; environment?: SceneEnvironment | null; warnings: EvaluationWarning[]; bytes: number; sent_ms: number; stats: EvaluationStats }
  | { kind: 'failure'; token: Token; error: { code: string; message: string } }
  | { kind: 'crash'; token: Token; error: { code: string; message: string } };

export function sameToken(a: Token | null, b: Token): boolean {
  return !!a && a.document === b.document && a.revision === b.revision && a.request === b.request
    && a.generation === b.generation && a.quality === b.quality && a.hash === b.hash;
}
