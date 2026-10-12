import type { Document } from './generated';

export type Quality = 'preview' | 'final';
export type Token = {
  document: string; revision: number; request: number; generation: number; quality: Quality; hash: string;
};
export type Candidate = { document: Document; hash: string; pending: boolean };
export type Job = { token: Token; document: Document; test?: 'spin' | 'crash' };
export type MeshPart = { node: string; name: string; volume: number; positions: Float32Array; indices: Uint32Array };
export type EvaluationStats = {
  evaluated_nodes: number; cache_hits: number; cache_bytes: number; elapsed_ms: number; memory_bytes: number;
};
export type WorkerMessage =
  | { kind: 'ready'; startup_ms: number }
  | { kind: 'cancelled'; token: Token }
  | { kind: 'result'; token: Token; parts: MeshPart[]; bytes: number; sent_ms: number; stats: EvaluationStats }
  | { kind: 'failure'; token: Token; error: { code: string; message: string } }
  | { kind: 'crash'; token: Token; error: { code: string; message: string } };

export function sameToken(a: Token | null, b: Token): boolean {
  return !!a && a.document === b.document && a.revision === b.revision && a.request === b.request
    && a.generation === b.generation && a.quality === b.quality && a.hash === b.hash;
}
