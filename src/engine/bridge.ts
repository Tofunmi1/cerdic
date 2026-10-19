import type { Command, CommandBatch, Diagnostic, Document, Outcome } from './generated';

export type DocumentView = {
  id: string; revision: number; nodes: number; can_undo: boolean; can_redo: boolean;
  read_only: boolean; saved_generation: number | null; issues: Diagnostic[]; document: Document;
  pending_document?: Document | null;
};
export type StageResult = Outcome | { pending: Outcome };
export interface Runtime {
  default(): Promise<{ memory: WebAssembly.Memory }>;
  start(canvas: string, scene: string): void;
  status(): string;
  eval_tangents(positions: Float32Array, normals: Float32Array, uvs: Float32Array, indices: Uint32Array): Float32Array;
  measure_mesh(positions: Float32Array, indices: Uint32Array, uvs: Float32Array, detail: string): string;
  doc_open(name: string, seed: number): string;
  doc_reset(name: string, seed: number): string;
  doc_state(): string;
  doc_apply(batch: string): string;
  doc_undo(): string;
  doc_redo(): string;
  doc_save(): string;
  doc_operators(): string;
  doc_candidate_state(): string;
  doc_parse_text(text: string): string;
  creature_make(request: string): string;
  doc_text(): string;
  doc_stage(batch: string): string;
  doc_stage_undo(): string;
  doc_stage_redo(): string;
  eval_bind(token: string): boolean;
  eval_reject(token: string, code: string, message: string): boolean;
  eval_projection_begin(token: string): boolean;
  eval_projection_instances(token:string,group:number,node:string,matrices:Float32Array,indices:Uint32Array):boolean;
  eval_projection_chunk_group(token:string,group:number,node:string,name:string,vertices:Float32Array):boolean;
  eval_projection_material_region(token: string, node: string, region: number, material: string): boolean;
  eval_projection_maps(token: string, node: string, region: number, albedo: Uint8Array, normal: Uint8Array, metallicRoughness: Uint8Array, emissive: Uint8Array): boolean;
  eval_projection_animation(token: string, node: string, tracks: string): boolean;
  viewer_animation(playing: boolean, seconds: number): void;
  viewer_animation_speed(speed: number): void;
  eval_projection_behavior(token: string, node: string, behavior: string): boolean;
  eval_projection_parent(token: string, node: string, parent: string): boolean;
  eval_projection_ik(token: string, node: string, chain: string): boolean;
  eval_projection_body(token: string, node: string, body: string, points: Float32Array, triangles: Uint32Array): boolean;
  eval_projection_skin(token: string, node: string, bones: string, indices: Uint32Array, joints: Uint32Array, weights: Float32Array, dual: boolean): boolean;
  viewer_play_mode(on: boolean): void;
  viewer_press(node: string): void;
  viewer_flag(name: string, on: boolean): void;
  viewer_behaviors(): string;
  eval_projection_lights(token: string, lights: string): boolean;
  eval_projection_environment(token: string, environment: string): boolean;
  eval_projection_chunk_part(token: string, group: number, region: number, node: string, name: string, vertices: Float32Array, textured: boolean): boolean;
  eval_projection_material(token: string, node: string, material: string): boolean;
  eval_projection_chunk(token: string, node: string, name: string, vertices: Float32Array): boolean;
  eval_projection_finish(token: string): boolean;
  eval_projection_status(): string;
  viewer_configure(enabled: boolean, fov: number, stereo: boolean, speed: number): void;
  viewer_move(right: number, up: number, forward: number, sprint: boolean): void;
  viewer_look(x: number, y: number): void;
  viewer_set_locked(locked: boolean): void;
  viewer_look_at(px: number, py: number, pz: number, tx: number, ty: number, tz: number): boolean;
  viewer_zoom(amount: number): void;
  viewer_orbit(dx: number, dy: number): void;
  viewer_frame(minX: number, minY: number, minZ: number, maxX: number, maxY: number, maxZ: number): boolean;
  xr_supported(): Promise<boolean>;
  enter_xr(canvas: string): Promise<void>;
  exit_xr(): Promise<void>;
}

export function documentBridge(runtime: Runtime, notify: () => void, evaluate?: () => void) {
  const state = (): DocumentView => JSON.parse(runtime.doc_state());
  return {
    state,
    apply(batch: CommandBatch): StageResult {
      if (!Number.isSafeInteger(batch.base_revision)) throw new Error('Unsafe document revision');
      const result: StageResult = JSON.parse(evaluate ? runtime.doc_stage(JSON.stringify(batch)) : runtime.doc_apply(JSON.stringify(batch)));
      if ('pending' in result) evaluate?.();
      notify();
      return result;
    },
    send(commands: Command[], label: string): StageResult {
      const current = state();
      return this.apply({ doc: current.id, base_revision: current.pending_document?.revision ?? current.revision,
        transaction: `t_${crypto.randomUUID()}`, label, commands });
    },
    // The text form: `{ diagnostics }` when it does not read, else the staged outcome of its commands.
    sendText(text: string, label: string): StageResult | { diagnostics: Diagnostic[] } {
      const read = JSON.parse(runtime.doc_parse_text(text)) as { commands?: Command[]; diagnostics?: Diagnostic[] };
      if (!read.commands) return { diagnostics: read.diagnostics ?? [] };
      return this.send(read.commands, label);
    },
    text(): { text?: string; error?: Diagnostic } { return JSON.parse(runtime.doc_text()); },
    undo() { const result: unknown = JSON.parse(evaluate ? runtime.doc_stage_undo() : runtime.doc_undo()); if (result) evaluate?.(); notify(); return result; },
    redo() { const result: unknown = JSON.parse(evaluate ? runtime.doc_stage_redo() : runtime.doc_redo()); if (result) evaluate?.(); notify(); return result; },
    save() { const result: unknown = JSON.parse(runtime.doc_save()); notify(); return result; },
  };
}
