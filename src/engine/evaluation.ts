import type { Runtime } from './bridge';
import { uploadResult } from './mesh-upload';
import type { Candidate, EvaluationStats, Job, Quality, Token, WorkerMessage, MeshPart } from './worker-types';
import { sameToken } from './worker-types';

function boundsOf(parts: MeshPart[]) {
  const min = [Infinity, Infinity, Infinity], max = [-Infinity, -Infinity, -Infinity];
  for (const part of parts) {
    const localMin=[Infinity,Infinity,Infinity], localMax=[-Infinity,-Infinity,-Infinity];
    for (let i = 0; i < part.positions.length; i += 3) {
      for (let k = 0; k < 3; k++) { localMin[k] = Math.min(localMin[k], part.positions[i + k]); localMax[k] = Math.max(localMax[k], part.positions[i + k]); }
    }
    if (part.transforms?.length) {
      for (let i=0;i<part.transforms.length;i+=16) {
        const m=part.transforms;
        for (let corner=0;corner<8;corner++) {
          const p=[0,1,2].map(k=>(corner&(1<<k)) ? localMax[k] : localMin[k]);
          for (let k=0;k<3;k++) { const v=m[i+k]*p[0]+m[i+4+k]*p[1]+m[i+8+k]*p[2]+m[i+12+k]; min[k]=Math.min(min[k],v);max[k]=Math.max(max[k],v); }
        }
      }
    } else {
      for (let k=0;k<3;k++) {min[k]=Math.min(min[k],localMin[k]);max[k]=Math.max(max[k],localMax[k]);}
    }
  }
  return Number.isFinite(min[0]) ? { min, max } : null;
}

export class EvaluationHost {
  /** Named sockets on the last accepted outputs, including placement data for selected instances. */
  socketParts: Pick<MeshPart,'node'|'name'|'frames'|'transforms'|'instance_indices'>[] = [];
  readonly state = {
    worker: 'booting', generation: 0, queue: 0, revision: null as number | null,
    lastFailure: null as { code: string; message: string } | null,
    recoveries: 0, recoveryMs: 0, staleResults: 0, startupMs: 0, transferBytes: 0, transferMs: 0, uploadMs: 0,
    stats: null as EvaluationStats | null,
  };
  private worker: Worker | null = null;
  private active: Job | null = null;
  private queued: Job | null = null;
  private latest: Token | null = null;
  private sequence = 0;
  private ready = false;
  private disposed = false;
  private hard?: ReturnType<typeof setTimeout>;
  private soft?: ReturnType<typeof setTimeout>;
  private restarts: number[] = [];
  private recoveryStarted: number | null = null;
  private testMode = new URLSearchParams(location.search).get('evaltest') === '1';

  /** Bounds of the last scene the renderer accepted, in metres. */
  bounds: { min: number[]; max: number[] } | null = null;

  constructor(private runtime: Runtime, private notify: () => void,
    private resources: { workers: number },
    private options: { workerUrl?: string; onProjected?: (bounds: { min: number[]; max: number[] }) => void } = {}) { this.start(); }

  request(quality: Quality = 'preview', test?: Job['test']) {
    if (this.disposed || this.state.worker === 'failed') return;
    const candidate = JSON.parse(this.runtime.doc_candidate_state()) as Candidate;
    if (!candidate.document.nodes.length && !candidate.pending) return;
    const token: Token = { document: candidate.document.id, revision: candidate.document.revision,
      request: ++this.sequence, generation: this.state.generation, quality, hash: candidate.hash };
    if (!this.runtime.eval_bind(JSON.stringify(token))) return;
    this.latest = token;
    this.queued = { token, document: candidate.document, ...(this.testMode && test ? { test } : {}) };
    this.state.queue = 1;
    if (this.active) this.worker?.postMessage({ kind: 'cancel', token: this.active.token });
    else this.dispatch();
    this.notify();
  }

  private start() {
    this.state.generation++; this.ready = false; this.state.worker = 'booting';
    const url = `${this.options.workerUrl ?? '/engine/eval-worker.js'}${this.testMode ? '?test=1' : ''}`;
    this.worker = new Worker(url, { type: 'module' }); this.resources.workers = 1;
    const generation = this.state.generation;
    this.worker.onmessage = event => {
      if (generation !== this.state.generation) { this.state.staleResults++; return; }
      void this.message(event.data as WorkerMessage);
    };
    this.worker.onerror = event => { event.preventDefault(); this.failed('eval.worker_crash', event.message || 'Worker initialization failed'); };
    this.worker.onmessageerror = () => this.failed('eval.worker_crash', 'Worker message could not be decoded');
  }

  private dispatch() {
    if (!this.ready || !this.queued || this.active || this.disposed) return;
    const job = this.queued; this.queued = null; this.state.queue = 0;
    job.token.generation = this.state.generation;
    if (!this.runtime.eval_bind(JSON.stringify(job.token))) { this.state.staleResults++; return; }
    this.latest = job.token; this.active = job; this.state.worker = 'evaluating';
    this.worker?.postMessage({ kind: 'evaluate', ...job });
    this.soft = setTimeout(() => { this.state.worker = 'overdue'; this.notify(); }, job.token.quality === 'preview' ? 2000 : 30000);
    this.hard = setTimeout(() => this.failed('eval.worker_timeout', 'Evaluation exceeded its hard time limit'),
      job.token.quality === 'preview' ? 10000 : 60000);
    this.notify();
  }

  private async message(message: WorkerMessage) {
    if (this.disposed) return;
    if (message.kind === 'ready') {
      this.ready = true; this.state.startupMs = message.startup_ms; this.state.worker = 'idle';
      this.dispatch(); this.notify(); return;
    }
    if (!sameToken(this.active?.token ?? null, message.token)) { this.state.staleResults++; this.notify(); return; }
    if (message.kind === 'cancelled') { this.complete(); return; }
    if (message.kind === 'crash') { this.failed(message.error.code, message.error.message); return; }
    if (message.kind === 'failure') {
      this.runtime.eval_reject(JSON.stringify(message.token), message.error.code, message.error.message);
      if (sameToken(this.latest, message.token)) this.state.lastFailure = message.error;
      else this.state.staleResults++;
      this.complete(); return;
    }
    if (!sameToken(this.latest, message.token)) { this.state.staleResults++; this.complete(); return; }
    this.clearTimers(); this.state.worker = 'uploading'; this.notify();
    this.hard = setTimeout(() => this.failed('eval.projection_timeout', 'Scene upload exceeded 60 seconds'), 60000);
    this.state.transferBytes = message.bytes;
    this.state.transferMs = Math.max(0, performance.timeOrigin + performance.now() - message.sent_ms);
    this.state.stats = message.stats;
    const began = performance.now();
    let accepted: boolean;
    try {
      accepted = await uploadResult(this.runtime, message.token, message.parts,
        () => !this.disposed && sameToken(this.latest, message.token)
          && sameToken(this.active?.token ?? null, message.token));
    } catch (error) {
      if (sameToken(this.active?.token ?? null, message.token)) {
        this.failed('eval.projection_failed', error instanceof Error ? error.message : 'Scene upload failed');
      }
      return;
    }
    if (!sameToken(this.active?.token ?? null, message.token)) return;
    this.state.uploadMs = performance.now() - began;
    if (accepted) {
      this.state.revision = message.token.revision;
      this.bounds = boundsOf(message.parts);
      this.socketParts=message.parts.filter(part=>Object.keys(part.frames ?? {}).length>0)
        .map(({node,name,frames,transforms,instance_indices})=>({node,name,frames,transforms,instance_indices}));
      if (this.bounds) this.options.onProjected?.(this.bounds);
      if (this.recoveryStarted !== null) { this.state.recoveryMs = performance.now() - this.recoveryStarted; this.recoveryStarted = null; }
    }
    else if (sameToken(this.latest, message.token)) {
      this.failed('eval.projection_failed', 'The renderer rejected the evaluated scene');
      return;
    }
    else this.state.staleResults++;
    this.complete();
  }

  private clearTimers() { clearTimeout(this.hard); clearTimeout(this.soft); this.hard = undefined; this.soft = undefined; }
  private complete() {
    this.clearTimers(); this.active = null; this.state.worker = 'idle'; this.dispatch(); this.notify();
  }
  private failed(code: string, message: string) {
    if (this.disposed) return;
    const token = this.active?.token;
    if (token) this.runtime.eval_reject(JSON.stringify(token), code, message);
    this.state.lastFailure = { code, message }; this.clearTimers();
    this.worker?.terminate(); this.worker = null; this.resources.workers = 0;
    this.active = null; this.ready = false;
    const now = performance.now(); this.restarts = this.restarts.filter(time => now - time < 60000);
    if (this.restarts.length >= 3) { this.state.worker = 'failed'; this.queued = null; this.state.queue = 0; this.notify(); return; }
    this.restarts.push(now); this.state.recoveries++; this.recoveryStarted = now; this.start();
    this.request(this.queued?.token.quality ?? 'preview');
    this.notify();
  }

  dispose() {
    this.disposed = true; this.clearTimers(); this.worker?.terminate(); this.worker = null;
    this.active = null; this.queued = null; this.resources.workers = 0; this.state.worker = 'disposed';this.socketParts=[];
  }
}
