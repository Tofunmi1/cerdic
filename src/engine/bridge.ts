import type { Command, CommandBatch, Diagnostic, Document, Outcome } from './generated';

export type DocumentView = {
  id: string; revision: number; nodes: number; can_undo: boolean; can_redo: boolean;
  read_only: boolean; saved_generation: number | null; issues: Diagnostic[]; document: Document;
};
export interface Runtime {
  default(): Promise<{ memory: WebAssembly.Memory }>;
  start(canvas: string, scene: string): void;
  status(): string;
  doc_open(name: string, seed: number): string;
  doc_state(): string;
  doc_apply(batch: string): string;
  doc_undo(): string;
  doc_redo(): string;
  doc_save(): string;
  viewer_configure(enabled: boolean, fov: number, stereo: boolean, speed: number): void;
  viewer_move(right: number, up: number, forward: number, sprint: boolean): void;
  viewer_look(x: number, y: number): void;
  viewer_set_locked(locked: boolean): void;
  xr_supported(): Promise<boolean>;
  enter_xr(canvas: string): Promise<void>;
  exit_xr(): Promise<void>;
}

export function documentBridge(runtime: Runtime, notify: () => void) {
  const state = (): DocumentView => JSON.parse(runtime.doc_state());
  return {
    state,
    apply(batch: CommandBatch): Outcome {
      if (!Number.isSafeInteger(batch.base_revision)) throw new Error('Unsafe document revision');
      const result: Outcome = JSON.parse(runtime.doc_apply(JSON.stringify(batch)));
      notify();
      return result;
    },
    send(commands: Command[], label: string): Outcome {
      const current = state();
      return this.apply({ doc: current.id, base_revision: current.revision,
        transaction: `t_${crypto.randomUUID()}`, label, commands });
    },
    undo() { const result: unknown = JSON.parse(runtime.doc_undo()); notify(); return result; },
    redo() { const result: unknown = JSON.parse(runtime.doc_redo()); notify(); return result; },
    save() { const result: unknown = JSON.parse(runtime.doc_save()); notify(); return result; },
  };
}
