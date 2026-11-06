// The parts of the engine runtime the studio panels call beyond what src/engine/bridge.ts types: the quality and grading switches, the camera,
// selection and playback. They are the same functions the plain viewer page (engine/web/viewer.js) calls.
import { host } from '../engine/host';

export interface ViewRuntime {
  set_quality(tier: string): void;
  set_shadows(on: boolean): void;
  set_shadow_resolution(texels: number): void;
  set_antialiasing(on: boolean): void;
  set_effect(name: string, on: boolean): void;
  set_specular_aa(on: boolean): void;
  set_ssao(on: boolean): void;
  set_bloom(on: boolean): void;
  set_auto_exposure(on: boolean): void;
  set_grading_preset(name: string): void;
  set_grading(exposure: number, contrast: number, saturation: number, r: number, g: number, b: number): void;
  grading_status(): string;
  viewer_configure(enabled: boolean, fov: number, stereo: boolean, speed: number): void;
  viewer_set_lens(lens: string): void;
  viewer_reset(): void;
  viewer_project(x: number, y: number, z: number, width: number, height: number): string;
  viewer_gizmo(a: number, b: number, c: number, d: number, e: number, f: number, width: number, height: number): string;
  gizmo_begin(mode: string, axis: number, x: number, y: number, aspect: number, px: number, py: number, pz: number): boolean;
  gizmo_update(x: number, y: number, aspect: number, grid: number, angle: number, scale: number): string;
  gizmo_cancel(): void;
  viewer_zoom(amount: number): void;
  viewer_orbit(dx: number, dy: number): void;
  viewer_frame(a: number, b: number, c: number, d: number, e: number, f: number): boolean;
  viewer_animation(playing: boolean, seconds: number): void;
  viewer_animation_speed(speed: number): void;
  viewer_play_mode(on: boolean): void;
  viewer_behaviors(): string;
  viewer_physics(): string;
  animation_set_float(part: string, name: string, value: number): void;
  animation_set_bool(part: string, name: string, value: boolean): void;
  animation_trigger(part: string, name: string): void;
  selection_get(): string;
  selection_set(node: string, parts: string): boolean;
  selection_clear(): void;
  eval_projection_status(): string;
  doc_operators(): string;
  game_desktop_hand?(grip: boolean, trigger: boolean): void;
  status(): string;
}

/** The runtime with the viewer functions, or null before the engine has started. */
export const rt = (): ViewRuntime | null => (host.runtime as unknown as ViewRuntime | null);

/** One parameter of an operator, as the engine describes it. */
export type ParamSpec = {
  name: string; doc: string; default: unknown;
  ty: { type: 'num' | 'int' | 'bool' | 'choice' | 'str' | 'vec2' | 'vec3' | 'points2'; unit?: string; options?: string[] };
};
export type OperatorSpec = { id: string; doc: string; inputs: { name: string }[]; params: ParamSpec[] };

let cached: Map<string, OperatorSpec> | null = null;
/** Every operator the engine knows, by id (read once). */
export function operators(): Map<string, OperatorSpec> {
  if (cached) return cached;
  const runtime = rt();
  if (!runtime) return new Map();
  cached = new Map((JSON.parse(runtime.doc_operators()) as OperatorSpec[]).map(spec => [spec.id, spec]));
  return cached;
}
