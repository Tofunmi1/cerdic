import type { Command, CommandBatch, Diagnostic, Document, Outcome } from './generated';
import type { EvaluatorText, TextParse, TextRead } from './evaluator-text';

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
  game_load(text: string): string;
  game_status(): string;
  game_set_hands(json: string): boolean;
  game_effects_since(after: number): string;
  audio_sfx(kind: string, seed: number, intensity: number): Float32Array;
  audio_sfx_names(): string;
  eval_tangents(positions: Float32Array, normals: Float32Array, uvs: Float32Array, indices: Uint32Array): Float32Array;
  measure_mesh(positions: Float32Array, indices: Uint32Array, uvs: Float32Array, detail: string): string;
  survey_plan(options: string): string;
  bake_maps(lowPositions: Float32Array, lowIndices: Uint32Array, lowCornerUvs: Float32Array, highPositions: Float32Array, highIndices: Uint32Array, size: number, maps: string, cage: number, maxDistance: number, rays: number, bias: number): string;
  doc_open(name: string, seed: number): string;
  doc_reset(name: string, seed: number): string;
  doc_state(): string;
  doc_at_revision(revision: number): string;
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
  eval_projection_instances(token:string,group:number,node:string,matrices:Float32Array,indices:Uint32Array,hasRange:boolean,showFrom:number,showTo:number,showFade:number):boolean;
  eval_visible_range(part:number):Float32Array;
  eval_visible_fade(part:number):number;
  eval_projection_chunk_group(token:string,group:number,node:string,name:string,vertices:Float32Array):boolean;
  eval_projection_material_region(token: string, node: string, region: number, material: string): boolean;
  viewer_texture_update(node: string, region: number, slot: number, pixels: Uint8Array): boolean;
  game_save_request(): void;
  game_save_take(): string;
  game_save_load(text: string): string;
  game_save_result(): string;
  eval_projection_cloth(token: string, node: string, spec: string, points: Float32Array, corners: Uint32Array): boolean;
  eval_projection_fluid(token: string, node: string, spec: string): boolean;
  eval_projection_sim(token: string, node: string, spec: string, points: Float32Array, corners: Uint32Array): boolean;
  viewer_sim(): string;
  viewer_cloth(): string;
  viewer_fluid(): string;
  eval_projection_maps(token: string, node: string, region: number, albedo: Uint8Array, normal: Uint8Array, metallicRoughness: Uint8Array, emissive: Uint8Array): boolean;
  eval_projection_animation(token: string, node: string, tracks: string): boolean;
  viewer_animation(playing: boolean, seconds: number): void;
  animation_set_float(part: string, name: string, value: number): void;
  animation_set_bool(part: string, name: string, value: boolean): void;
  animation_trigger(part: string, name: string): void;
  world_call(method: string, params: string): number;
  world_take(id: number): string | undefined;
  world_events_since(after: number): string;
  viewer_animation_speed(speed: number): void;
  eval_projection_behavior(token: string, node: string, behavior: string): boolean;
  eval_projection_machine(token: string, node: string, machine: string): boolean;
  eval_projection_parent(token: string, node: string, parent: string): boolean;
  eval_projection_ik(token: string, node: string, chain: string): boolean;
  eval_projection_fx(token: string, node: string, effect: string): boolean;
  eval_projection_ragdoll(token: string, node: string, ragdoll: string): boolean;
  eval_projection_body(token: string, node: string, body: string, points: Float32Array, triangles: Uint32Array): boolean;
  eval_projection_skin(token: string, node: string, bones: string, indices: Uint32Array, joints: Uint32Array, weights: Float32Array, dual: boolean): boolean;
  viewer_play_mode(on: boolean): void;
  viewer_press(node: string): void;
  viewer_flag(name: string, on: boolean): void;
  viewer_behaviors(): string;
  eval_projection_lights(token: string, lights: string): boolean;
  eval_projection_environment(token: string, environment: string): boolean;
  eval_projection_chunk_part(token: string, group: number, region: number, node: string, name: string, vertices: Float32Array, layout: number): boolean;
  eval_projection_material(token: string, node: string, material: string): boolean;
  eval_projection_chunk(token: string, node: string, name: string, vertices: Float32Array): boolean;
  eval_projection_finish(token: string): boolean;
  eval_projection_finish_draft(token: string): boolean;
  eval_projection_status(): string;
  viewer_configure(enabled: boolean, fov: number, stereo: boolean, speed: number): void;
  viewer_move(right: number, up: number, forward: number, sprint: boolean): void;
  viewer_look(x: number, y: number): void;
  viewer_set_locked(locked: boolean): void;
  viewer_look_at(px: number, py: number, pz: number, tx: number, ty: number, tz: number): boolean;
  viewer_pose(): string;
  viewer_zoom(amount: number): void;
  viewer_orbit(dx: number, dy: number): void;
  viewer_frame(minX: number, minY: number, minZ: number, maxX: number, maxY: number, maxZ: number): boolean;
  xr_supported(): Promise<boolean>;
  enter_xr(canvas: string): Promise<void>;
  exit_xr(): Promise<void>;
}

export function documentBridge(runtime: Runtime, notify: () => void, evaluate?: () => void, textTools?: EvaluatorText) {
  const state = (): DocumentView => JSON.parse(runtime.doc_state());
  return {
    state,
    atRevision(revision: number): { document: Document; lines: Record<string, string>; error?: never } | { error: string; document?: never; lines?: never } {
      return JSON.parse(runtime.doc_at_revision(revision));
    },
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
    async parseText(text: string): Promise<TextParse> {
      const view = state();
      return textTools ? textTools.request(view.pending_document ?? view.document, 'parse', text)
        : JSON.parse(runtime.doc_parse_text(text)) as TextParse;
    },
    async sendText(text: string, label: string): Promise<StageResult | { diagnostics: Diagnostic[] }> {
      const read = await this.parseText(text);
      if (!read.commands) return { diagnostics: read.diagnostics ?? (read.error ? [read.error] : []) };
      return this.send(read.commands, label);
    },
    text(): { text?: string; error?: Diagnostic } { return JSON.parse(runtime.doc_text()); },
    async readText(format: 'compact' | 'canonical' = 'compact'): Promise<TextRead> {
      if (format === 'canonical') return this.text();
      if (!textTools) throw new Error('the evaluator text service is not available');
      const view = state();
      return textTools.request(view.pending_document ?? view.document, 'compact');
    },
    undo() { const result: unknown = JSON.parse(evaluate ? runtime.doc_stage_undo() : runtime.doc_undo()); if (result) evaluate?.(); notify(); return result; },
    redo() { const result: unknown = JSON.parse(evaluate ? runtime.doc_stage_redo() : runtime.doc_redo()); if (result) evaluate?.(); notify(); return result; },
    save() { const result: unknown = JSON.parse(runtime.doc_save()); notify(); return result; },
  };
}
