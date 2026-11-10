import type { Runtime } from './bridge';
import { lastRefusal, uploadResult } from './mesh-upload';
import type { Attachments, Candidate, EvaluationStats, EvaluationWarning, GameInfo, Job, Quality, Token, WorkerMessage, MeshPart } from './worker-types';
import { sameToken } from './worker-types';
import { EvaluatorText } from './evaluator-text';

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

/**
 * The part of a scene worth looking at. A scene with a sea, a terrain or a big ground plane would otherwise be framed as one huge box and the camera would stand
 * back until the subject is a dot. So background parts are left out: those named like one (sea, ocean, ground, land, terrain, lawn, cliff, mountains, backdrop,
 * outer) and, in one pass, any part whose box is over half the size of what is left and over three times the median part (a forest field, a sky dome).
 * If nothing would remain, the whole scene is used.
 */
export type Box = { min: number[]; max: number[] };
export function subjectBounds(parts: Array<{ name: string; min: number[] | null; max: number[] | null }>): Box | null {
  const boxes = parts.filter((p): p is { name: string; min: number[]; max: number[] } => !!p.min && !!p.max);
  if (boxes.length === 0) return null;
  const diag = (b: Box) => Math.hypot(b.max[0] - b.min[0], b.max[1] - b.min[1], b.max[2] - b.min[2]);
  const union = (list: Box[]): Box => ({
    min: [0, 1, 2].map(k => Math.min(...list.map(b => b.min[k]))),
    max: [0, 1, 2].map(k => Math.max(...list.map(b => b.max[k]))),
  });
  const background = /(^|_)(sea|ocean|ground|land|terrain|lawn|cliffs?|mountains?|backdrop|horizon|outer)(_|\d|$)/i;
  const named = boxes.filter(b => !background.test(b.name));
  const pool = named.length > 0 ? named : boxes;
  const sizes = pool.map(diag).sort((a, b) => a - b);
  const median = sizes[Math.floor(sizes.length / 2)] ?? 0;
  const whole = diag(union(pool));
  const kept = pool.filter(b => !(diag(b) > 0.5 * whole && diag(b) > 3 * median));
  return union(kept.length > 0 ? kept : pool);
}

/** What the last accepted evaluation measured for one output part, in metres and cubic metres. */
export type PartMeasure = {
  node: string; name: string; volume: number; triangles: number;
  min: number[] | null; max: number[] | null; size: number[] | null;
};

export class EvaluationHost {
  readonly text = new EvaluatorText(() => this.worker);
  /** Per-part measurements of the last accepted result (read by the MCP `measure` tool). */
  measures: PartMeasure[] = [];
  /** The meshes of the last accepted result, kept so the model can be exported (`exportGlb`). */
  parts: MeshPart[] = [];
  /** Surface gap report from the last accepted final quality evaluation. */
  attachments: Attachments | null = null;
  /** Named sockets on the last accepted outputs, including placement data for selected instances. */
  socketParts: Pick<MeshPart,'node'|'name'|'frames'|'transforms'|'instance_indices'>[] = [];
  readonly state = {
    worker: 'booting', generation: 0, queue: 0, revision: null as number | null,
    refining: false,
    /** Nodes the running evaluation has finished, out of the document's nodes (0 of 0 when none is running). */
    progress: { evaluated: 0, total: 0 },
    lastFailure: null as { code: string; message: string; node?: string; related?: string[] } | null,
    /** Whether `lastFailure` still describes the document on screen (false once a later evaluation succeeded). */
    failureCurrent: false,
    recoveries: 0, recoveryMs: 0, staleResults: 0, startupMs: 0, transferBytes: 0, transferMs: 0, uploadMs: 0,
    stats: null as EvaluationStats | null,
    warnings: [] as EvaluationWarning[],
  };
  /** The game of the last accepted result (ADR 0062), as the evaluator printed it. */
  game: GameInfo = { present: false };
  /** The parts of the running evaluation finished so far, shown as a draft while the rest evaluates (`draft` messages from the worker). */
  private draftParts = new Map<string, MeshPart[]>();
  private draftDirty = false;
  private draftStop = false;
  private draftUpload: Promise<void> | null = null;
  private playtests = new Map<number, (report: string) => void>();
  private playtestId = 0;
  private worker: Worker | null = null;
  private active: Job | null = null;
  private queued: Job | null = null;
  private latest: Token | null = null;
  private sequence = 0;
  private ready = false;
  private disposed = false;
  private hard?: ReturnType<typeof setTimeout>;
  private soft?: ReturnType<typeof setTimeout>;
  private refineTimer?: ReturnType<typeof setTimeout>;
  private restarts: number[] = [];
  private recoveryStarted: number | null = null;
  private testMode = new URLSearchParams(location.search).get('evaltest') === '1';

  /** Bounds of the last scene the renderer accepted, in metres. */
  bounds: { min: number[]; max: number[] } | null = null;
  /** The box worth framing: `bounds` without the background (sea, terrain, ground, a forest field); F, R and the first view use it. */
  subject: { min: number[]; max: number[] } | null = null;
  boundsByNode: Record<string, { min: number[]; max: number[] }> = {};
  /** Small view-only samples from the last accepted mesh, kept separate from document state. */
  overlaysByNode: Record<string, { wire: number[][][]; normals: number[][][] }> = {};

  overlayForNodes(nodes: string[]) {
    const samples = nodes.map(node => this.overlaysByNode[node]).filter(Boolean);
    return { wire: samples.flatMap(sample => sample.wire), normals: samples.flatMap(sample => sample.normals) };
  }

  /** Bounds of the accepted evaluated parts named by the current selection. */
  boundsForNodes(nodes: string[]) {
    const selected = nodes.map(node => this.boundsByNode[node]).filter(Boolean);
    if (!selected.length) return null;
    return {
      min: [0, 1, 2].map(axis => Math.min(...selected.map(bounds => bounds.min[axis]))),
      max: [0, 1, 2].map(axis => Math.max(...selected.map(bounds => bounds.max[axis]))),
    };
  }

  /** Position of a named result frame in its accepted instance placement, if present. */
  socketPosition(node: string, name: string) {
    const part = this.socketParts.find((candidate) => candidate.node === node && candidate.frames?.[name]);
    const frame = part?.frames?.[name];
    if (!part || !frame) return null;
    let position = [...frame.position];
    if (part.transforms?.length) {
      const offset = 0;
      const matrix = part.transforms;
      position = [0, 1, 2].map(axis => matrix[offset + axis] * frame.position[0]
        + matrix[offset + 4 + axis] * frame.position[1]
        + matrix[offset + 8 + axis] * frame.position[2]
        + matrix[offset + 12 + axis]);
    }
    return position;
  }

  /** World-space frames (origin and three unit axes) of the named sockets on `nodes`, in the first accepted
   * placement of each part. The axes are the frame's orientation (a unit quaternion, X Y Z W) carried
   * through that placement, so a triad drawn from them shows what an attachment would match. */
  socketFrames(nodes: string[]) {
    const out: { part: string; name: string; position: number[]; axes: number[][] }[] = [];
    for (const part of this.socketParts) {
      if (!nodes.includes(part.node)) continue;
      const matrix = part.transforms?.length ? part.transforms : null;
      const place = (v: number[]) => matrix ? [0, 1, 2].map(axis => matrix[axis] * v[0] + matrix[4 + axis] * v[1] + matrix[8 + axis] * v[2]) : v;
      for (const [name, frame] of Object.entries(part.frames ?? {})) {
        const position = this.socketPosition(part.node, name);
        if (!position) continue;
        const [x, y, z, w] = frame.rotation;
        const turn = (v: number[]) => {
          const t = [2 * (y * v[2] - z * v[1]), 2 * (z * v[0] - x * v[2]), 2 * (x * v[1] - y * v[0])];
          return [v[0] + w * t[0] + (y * t[2] - z * t[1]), v[1] + w * t[1] + (z * t[0] - x * t[2]), v[2] + w * t[2] + (x * t[1] - y * t[0])];
        };
        const axes = [[1, 0, 0], [0, 1, 0], [0, 0, 1]].map(unit => {
          const world = place(turn(unit));
          const length = Math.hypot(world[0], world[1], world[2]) || 1;
          return world.map(component => component / length);
        });
        out.push({ part: part.node, name, position, axes });
      }
    }
    return out;
  }

  constructor(private runtime: Runtime, private notify: () => void,
    private resources: { workers: number },
    private options: { workerUrl?: string; onProjected?: (bounds: { min: number[]; max: number[] }) => void; onScripts?: (scripts: Array<{ name: string; source: string; enabled: boolean; node: string }>) => void } = {}) { this.start(); }

  request(quality: Quality = 'preview', test?: Job['test']) {
    if (this.disposed || this.state.worker === 'failed') return;
    clearTimeout(this.refineTimer); this.refineTimer = undefined; this.state.refining = quality === 'final';
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

  /** Plays a game text headless in the evaluator worker and answers its report (JSON text). Policy: idle, random or seek:<hero tag>:<target tag>. */
  playtest(text: string, policy: string, seed: number, steps: number): Promise<string> {
    return new Promise((resolve, reject) => {
      if (!this.worker) { reject(new Error('the evaluator is not running')); return; }
      const id = ++this.playtestId;
      const timer = setTimeout(() => { this.playtests.delete(id); reject(new Error('the playtest took more than 20 seconds')); }, 20000);
      this.playtests.set(id, report => { clearTimeout(timer); resolve(report); });
      this.worker.postMessage({ kind: 'playtest', id, text, policy, seed, steps });
    });
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
    this.state.progress = { evaluated: 0, total: 0 };
    this.draftParts = new Map(); this.draftDirty = false; this.draftStop = false;
    this.worker?.postMessage({ kind: 'evaluate', ...job });
    this.soft = setTimeout(() => { this.state.worker = 'overdue'; this.notify(); }, job.token.quality === 'preview' ? 2000 : 30000);
    this.hard = setTimeout(() => this.failed('eval.worker_timeout', 'Evaluation exceeded its hard time limit'),
      job.token.quality === 'preview' ? 120000 : 600000);
    this.notify();
  }

  /** Shows every part finished so far, one upload at a time: while one is in flight, newer parts wait and go together in the next. */
  private showDraft(token: Token) {
    if (this.draftUpload) return;
    this.draftUpload = (async () => {
      try {
        while (this.draftDirty && !this.draftStop && !this.disposed && sameToken(this.active?.token ?? null, token)) {
          this.draftDirty = false;
          const current = () => !this.disposed && !this.draftStop && sameToken(this.latest, token) && sameToken(this.active?.token ?? null, token);
          // A draft shows shapes only: whatever ties a part to other parts (a parent, a skin, a leg chain, a body) or moves it waits for the finished scene.
          const shapes = [...this.draftParts.values()].flat().map(part => ({
            ...part, parent: undefined, skin: undefined, ik: undefined, ragdoll: undefined, fluid: undefined, sim: undefined, cloth: undefined, fx: undefined,
            body: undefined, behavior: undefined, state_machine: undefined, animation: undefined,
          } as MeshPart));
          await uploadResult(this.runtime, token, shapes, [], null, current, [], true);
        }
      } catch (error) { console.warn('a draft could not be shown (the finished scene still will be)', error); }
      finally { this.draftUpload = null; }
    })();
  }

  private async message(message: WorkerMessage) {
    if (this.disposed) return;
    if (message.kind === 'ready') {
      this.ready = true; this.state.startupMs = message.startup_ms; this.state.worker = 'idle';
      this.dispatch(); this.notify(); return;
    }
    if (message.kind === 'playtest_result') { this.playtests.get(message.id)?.(message.report); this.playtests.delete(message.id); return; }
    if (message.kind === 'document_text_result') { this.text.receive(message.id, message.reply); return; }
    if (!sameToken(this.active?.token ?? null, message.token)) { this.state.staleResults++; this.notify(); return; }
    if (message.kind === 'progress') { this.state.progress = { evaluated: message.evaluated, total: message.total }; this.notify(); return; }
    if (message.kind === 'draft') {
      this.state.progress = { evaluated: message.evaluated, total: message.total };
      // A draft is an output as built so far: it replaces the previous stage of the same output.
      const grown = new Map<string, MeshPart[]>();
      for (const part of message.parts) grown.set(part.node, [...(grown.get(part.node) ?? []), part]);
      for (const [node, parts] of grown) this.draftParts.set(node, parts);
      this.draftDirty = true;
      this.showDraft(message.token); this.notify(); return;
    }
    if (message.kind === 'cancelled') { this.complete(); return; }
    if (message.kind === 'crash') { this.failed(message.error.code, message.error.message); return; }
    if (message.kind === 'failure') {
      this.runtime.eval_reject(JSON.stringify(message.token), message.error.code, message.error.message);
      if (sameToken(this.latest, message.token)) {
        this.state.lastFailure = message.error; this.state.failureCurrent = true; this.state.warnings = [];
      }
      else this.state.staleResults++;
      this.complete(false, message.token); return;
    }
    if (!sameToken(this.latest, message.token)) { this.state.staleResults++; this.complete(); return; }
    // The finished scene replaces any draft: let the draft upload in flight end, and send no more.
    this.draftStop = true;
    if (this.draftUpload) await this.draftUpload;
    if (!sameToken(this.active?.token ?? null, message.token)) return;
    this.clearTimers(); this.state.worker = 'uploading'; this.notify();
    // A page on a slow (software) GPU, such as the test browser, may set `__veUploadTimeoutMs` to give texture baking longer.
    const uploadLimit = Number((globalThis as { __veUploadTimeoutMs?: number }).__veUploadTimeoutMs) || 60000;
    // The upload advances one frame at a time, and a background tab gets no frames, so time spent hidden is not counted against it.
    const expire = () => {
      if (typeof document !== 'undefined' && document.hidden) { this.hard = setTimeout(expire, uploadLimit); return; }
      this.failed('eval.projection_timeout', `Scene upload exceeded ${uploadLimit / 1000} seconds`);
    };
    this.hard = setTimeout(expire, uploadLimit);
    this.state.transferBytes = message.bytes;
    this.state.transferMs = Math.max(0, performance.timeOrigin + performance.now() - message.sent_ms);
    this.state.stats = message.stats;
    const began = performance.now();
    let accepted: boolean;
    const notes: EvaluationWarning[] = [];
    try {
      accepted = await uploadResult(this.runtime, message.token, message.parts, message.lights, message.environment ?? null,
        () => !this.disposed && sameToken(this.latest, message.token)
          && sameToken(this.active?.token ?? null, message.token), notes);
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
      this.state.warnings = [...message.warnings, ...notes];
      // `lastFailure` stays as history (the status panel keeps showing a crash after recovery); what
      // changes is whether it still describes the document on screen.
      this.state.failureCurrent = false;
      this.bounds = boundsOf(message.parts);
      this.game = message.game ?? { present: false };
      const gameError = this.runtime.game_load(this.game.present ? (this.game.text ?? '') : '');
      if (gameError) console.error(`Game load failed: ${gameError}`);
      // The scripts the document carries (`script.js` nodes): the page starts, restarts and stops its running scripts to match.
      this.options.onScripts?.(message.scripts ?? []);
      this.parts = message.parts;
      if (message.token.quality === 'final') this.attachments = message.attachments ?? null;
      this.measures = message.parts.map(part => {
        const bounds = boundsOf([part]);
        return {
          node: part.node, name: part.name, volume: part.volume, triangles: part.indices.length / 3,
          min: bounds?.min ?? null, max: bounds?.max ?? null,
          size: bounds ? bounds.max.map((v, k) => v - bounds.min[k]) : null,
        };
      });
      const byNode = new Map<string, MeshPart[]>();
      for (const part of message.parts) {
        const parts = byNode.get(part.node) ?? [];
        parts.push(part);
        byNode.set(part.node, parts);
      }
      this.boundsByNode = {};
      for (const [node, parts] of byNode) {
        const bounds = boundsOf(parts);
        if (bounds) this.boundsByNode[node] = bounds;
      }
      this.overlaysByNode = {};
      for (const part of message.parts) {
        const sample = meshOverlaySample(part);
        const existing: { wire: number[][][]; normals: number[][][] } = this.overlaysByNode[part.node] ?? { wire: [], normals: [] };
        existing.wire.push(...sample.wire);
        existing.normals.push(...sample.normals);
        this.overlaysByNode[part.node] = existing;
      }
      this.socketParts=message.parts.filter(part=>Object.keys(part.frames ?? {}).length>0)
        .map(({node,name,frames,transforms,instance_indices})=>({node,name,frames,transforms,instance_indices}));
      this.subject = subjectBounds(this.measures);
      if (this.bounds) this.options.onProjected?.(this.subject ?? this.bounds);
      if (this.recoveryStarted !== null) { this.state.recoveryMs = performance.now() - this.recoveryStarted; this.recoveryStarted = null; }
    }
    else if (sameToken(this.latest, message.token)) {
      const step = lastRefusal();
      this.failed('eval.projection_failed', `The renderer rejected the evaluated scene${step ? ` (${step} said no)` : ' (the scene was replaced while it uploaded)'}`);
      return;
    }
    else this.state.staleResults++;
    this.complete(accepted, message.token);
  }

  private clearTimers() { clearTimeout(this.hard); clearTimeout(this.soft); this.hard = undefined; this.soft = undefined; }
  private complete(accepted = false, token = this.active?.token) {
    this.clearTimers(); this.active = null; this.state.worker = 'idle';
    if (token && token.quality === 'preview' && (accepted || this.state.lastFailure?.code === 'budget.triangles')
      && sameToken(this.latest, token)) {
      this.state.refining = true;
      this.refineTimer = setTimeout(() => {
        this.refineTimer = undefined;
        if (!this.disposed && sameToken(this.latest, token)) this.request('final');
      }, 400);
    } else if (token && token.quality === 'final' && sameToken(this.latest, token)) this.state.refining = false;
    this.dispatch(); this.notify();
  }
  private failed(code: string, message: string) {
    if (this.disposed) return;
    this.text.fail(message);
    const token = this.active?.token;
    const retryQuality = this.queued?.token.quality ?? token?.quality ?? 'preview';
    if (token) this.runtime.eval_reject(JSON.stringify(token), code, message);
    this.state.lastFailure = { code, message }; this.state.failureCurrent = true; this.clearTimers();
    this.state.warnings = [];
    this.worker?.terminate(); this.worker = null; this.resources.workers = 0;
    this.active = null; this.ready = false;
    const now = performance.now(); this.restarts = this.restarts.filter(time => now - time < 60000);
    if (this.restarts.length >= 3) {
      this.state.worker = 'failed'; this.state.refining = false; this.queued = null; this.state.queue = 0; this.notify(); return;
    }
    this.restarts.push(now); this.state.recoveries++; this.recoveryStarted = now; this.start();
    this.request(retryQuality);
    this.notify();
  }

  dispose() {
    this.text.fail('the evaluator was disposed');
    this.disposed = true; this.clearTimers(); clearTimeout(this.refineTimer); this.refineTimer = undefined; this.worker?.terminate(); this.worker = null;
    this.active = null; this.queued = null; this.resources.workers = 0; this.state.worker = 'disposed';this.socketParts=[];this.boundsByNode={};this.overlaysByNode={};
  }
}

// Cap overlay geometry so a diagnostic view remains cheap even for dense evaluated meshes.
function meshOverlaySample(part: MeshPart): { wire: number[][][]; normals: number[][][] } {
  const wire: number[][][] = [], normals: number[][][] = [];
  const faces = Math.floor(part.indices.length / 3);
  const stride = Math.max(1, Math.ceil(faces / 64));
  const positions = part.positions;
  const transforms = part.transforms;
  const world = (index: number, instance: number) => {
    const point = [positions[index * 3], positions[index * 3 + 1], positions[index * 3 + 2]];
    if (!transforms?.length) return point;
    const at = instance * 16;
    return [0, 1, 2].map(axis => transforms[at + axis] * point[0] + transforms[at + 4 + axis] * point[1]
      + transforms[at + 8 + axis] * point[2] + transforms[at + 12 + axis]);
  };
  const instances = transforms?.length ? Math.min(transforms.length / 16, 4) : 1;
  for (let instance = 0; instance < instances; instance++) {
    for (let face = 0; face < faces; face += stride) {
      const ids = [part.indices[face * 3], part.indices[face * 3 + 1], part.indices[face * 3 + 2]];
      const points = ids.map(index => world(index, instance));
      wire.push([points[0], points[1]], [points[1], points[2]], [points[2], points[0]]);
      if (normals.length < 768) {
        const center = [0, 1, 2].map(axis => (points[0][axis] + points[1][axis] + points[2][axis]) / 3);
        const ab = points[1].map((value, axis) => value - points[0][axis]);
        const ac = points[2].map((value, axis) => value - points[0][axis]);
        const vector = [ab[1] * ac[2] - ab[2] * ac[1], ab[2] * ac[0] - ab[0] * ac[2], ab[0] * ac[1] - ab[1] * ac[0]];
        const length = Math.hypot(...vector) || 1;
        const end = center.map((value, axis) => value + vector[axis] / length * 0.06);
        normals.push([center, end]);
      }
    }
  }
  return { wire, normals };
}
