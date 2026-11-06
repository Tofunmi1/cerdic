// The transform gizmo: move, rotate or scale the selected node's result by dragging a handle in the view. It is the plain viewer's gizmo, ported:
// the engine does the geometry of the handles and the drag (`viewer_gizmo`, `gizmo_begin`, `gizmo_update`), and releasing the handle sends one typed
// command batch to the document (a transform node inserted after the selected node), so it is undoable like any other edit.
import { useEffect, useRef, useState } from 'react';
import type { ReactElement } from 'react';
import { Move3D, Rotate3D, Scaling } from 'lucide-react';
import { host } from '../engine/host';
import { insertTransformBatch, viewportEditConflictReason } from '../../engine/web/panels/transform-commands.js';
import { partsFor } from '../../engine/web/panels/graph.js';
import { rt } from './engine-api';

export type Mode = 'translate' | 'rotate' | 'scale';
export type Snap = { grid: number; angle: number; scale: number };
type Bounds = { min: number[]; max: number[] };
type Geometry = { pivot: number[]; axes: { axis: number; start: number[]; end: number[] }[]; rings: number[][][]; bounds: number[][][] };
type Delta = { kind: 'translate'; value: number[] } | { kind: 'rotate'; radians: number; axis?: number[] } | { kind: 'scale'; factor: number; axis?: number[] };
type Drag = { pointerId: number; axis: number; pivot: number[]; bounds: Bounds; revision: number; last: Delta | null };

const unit = (axis: number): number[] => [0, 1, 2].map(i => (i === axis ? 1 : 0));

/** Where a point of the box goes under the drag so far, to draw the box as it will be. */
function moved(point: number[], delta: Delta, axis: number, pivot: number[]): number[] {
  if (delta.kind === 'translate') return point.map((v, i) => v + delta.value[i]);
  const offset = point.map((v, i) => v - pivot[i]);
  if (delta.kind === 'scale') return offset.map((v, i) => pivot[i] + v * (i === axis ? delta.factor : 1));
  const a = unit(axis), c = Math.cos(delta.radians), s = Math.sin(delta.radians);
  const dot = a.reduce((sum, v, i) => sum + v * offset[i], 0);
  const cross = [a[1] * offset[2] - a[2] * offset[1], a[2] * offset[0] - a[0] * offset[2], a[0] * offset[1] - a[1] * offset[0]];
  return offset.map((v, i) => pivot[i] + v * c + cross[i] * s + a[i] * dot * (1 - c));
}

export function GizmoBar({ mode, setMode, snap, setSnap, status }: { mode: Mode; setMode: (m: Mode) => void; snap: Snap; setSnap: (s: Snap) => void; status: string }) {
  const tools: [Mode, string, typeof Move3D][] = [['translate', 'Move', Move3D], ['rotate', 'Rotate', Rotate3D], ['scale', 'Scale', Scaling]];
  const field = (label: string, key: keyof Snap, step: number) => (
    <label className="snap">{label}<input className="num" type="number" min={0} step={step} value={snap[key]} onChange={e => setSnap({ ...snap, [key]: Math.max(0, Number(e.target.value)) })} /></label>
  );
  return (
    <div className="gizmobar glass" role="toolbar" aria-label="Transform tool">
      <div className="segmented" role="group">
        {tools.map(([id, name, Icon]) => <button key={id} type="button" aria-pressed={mode === id} onClick={() => setMode(id)}><Icon size={13} style={{ marginRight: 5, verticalAlign: -2 }} />{name}</button>)}
      </div>
      {field('Grid m', 'grid', 0.01)}{field('Angle °', 'angle', 1)}{field('Step', 'scale', 0.01)}
      {status && <span className="gizmostatus">{status}</span>}
    </div>
  );
}

export function Gizmo({ node, mode, snap, onStatus }: { node: string; mode: Mode; snap: Snap; onStatus: (text: string) => void }) {
  const [, tick] = useState(0);
  const [preview, setPreview] = useState<{ delta: Delta; drag: Drag } | null>(null);
  const drag = useRef<Drag | null>(null);
  const svg = useRef<SVGSVGElement>(null);

  // The handles follow the camera, so they are laid out again a dozen times a second (and straight away when the drag ends).
  useEffect(() => {
    const timer = setInterval(() => { if (!drag.current) tick(n => n + 1); }, 80);
    // Escape abandons a drag in progress; nothing was committed during it.
    const escape = (event: KeyboardEvent) => {
      if (event.code !== 'Escape' || !drag.current) return;
      drag.current = null; rt()?.gizmo_cancel(); setPreview(null); onStatus('Drag cancelled.');
      event.preventDefault(); event.stopPropagation();
    };
    window.addEventListener('keydown', escape, true);
    return () => { clearInterval(timer); window.removeEventListener('keydown', escape, true); };
  }, [onStatus]);

  const runtime = rt();
  const evaluation = host.evaluation;
  const rect = host.canvas.getBoundingClientRect();
  const doc = host.doc?.state().document;
  if (!runtime || !evaluation || !doc || !rect.width || !rect.height) return null;
  const selected = JSON.parse(runtime.selection_get()) as { node: string | null; instance: number | null };
  if (selected.instance !== null) return null;
  const boxes = (partsFor(doc, node) as string[]).map(id => evaluation.boundsByNode[id]).filter(Boolean);
  if (!boxes.length) return null;
  const bounds: Bounds = {
    min: [0, 1, 2].map(k => Math.min(...boxes.map(b => b.min[k]))), max: [0, 1, 2].map(k => Math.max(...boxes.map(b => b.max[k]))),
  };
  let geometry: Geometry | null = null;
  try {
    const raw = runtime.viewer_gizmo(bounds.min[0], bounds.min[1], bounds.min[2], bounds.max[0], bounds.max[1], bounds.max[2], rect.width, rect.height);
    geometry = raw ? JSON.parse(raw) as Geometry : null;
  } catch { geometry = null; }
  if (!geometry) return null;

  const pointer = (event: React.PointerEvent) => ({
    x: ((event.clientX - rect.left) / rect.width) * 2 - 1, y: 1 - ((event.clientY - rect.top) / rect.height) * 2, aspect: rect.width / rect.height,
  });
  const down = (axis: number) => (event: React.PointerEvent) => {
    if (event.button !== 0 || document.pointerLockElement === host.canvas || !geometry) return;
    const current = host.doc!.state();
    if (current.pending_document) { onStatus('The previous edit is still being evaluated; try again when it finishes.'); return; }
    const p = pointer(event);
    if (!runtime.gizmo_begin(mode, axis, p.x, p.y, p.aspect, geometry.pivot[0], geometry.pivot[1], geometry.pivot[2])) return;
    drag.current = { pointerId: event.pointerId, axis, pivot: geometry.pivot, bounds, revision: current.revision, last: null };
    (event.target as Element).setPointerCapture(event.pointerId);
    onStatus(`${mode} ${'XYZ'[axis]} · release to commit · Esc to cancel`);
    event.preventDefault(); event.stopPropagation();
  };
  const move = (event: React.PointerEvent) => {
    const active = drag.current;
    if (!active || active.pointerId !== event.pointerId) return;
    if (host.doc!.state().revision !== active.revision) { cancel('The document changed during the drag; discarded.'); return; }
    const p = pointer(event);
    const raw = runtime.gizmo_update(p.x, p.y, p.aspect, snap.grid, (snap.angle * Math.PI) / 180, snap.scale);
    if (!raw) return;
    const delta = JSON.parse(raw) as Delta;
    active.last = delta;
    setPreview({ delta, drag: active });
    onStatus(delta.kind === 'translate' ? `Move · ${delta.value.map(v => v.toFixed(3)).join(', ')} m`
      : delta.kind === 'rotate' ? `Rotate · ${((delta.radians * 180) / Math.PI).toFixed(1)}°` : `Scale · ${delta.factor.toFixed(3)}×`);
    event.preventDefault(); event.stopPropagation();
  };
  const cancel = (message: string) => {
    if (!drag.current) return;
    drag.current = null; runtime.gizmo_cancel(); setPreview(null); onStatus(message);
  };
  const up = (event: React.PointerEvent) => {
    const active = drag.current;
    if (!active || active.pointerId !== event.pointerId) return;
    drag.current = null; runtime.gizmo_cancel(); setPreview(null);
    const delta = active.last;
    if (!delta) { onStatus(''); return; }
    const current = host.doc!.state();
    const conflict = viewportEditConflictReason(current, { node }, node, active.revision);
    if (conflict) { onStatus(conflict === 'pending' ? 'The previous edit is still being evaluated; the move was discarded.' : 'The document changed during the drag; the move was discarded.'); return; }
    const edit = delta.kind === 'translate' ? { mode: 'translate', delta: delta.value, pivot: active.pivot }
      : delta.kind === 'rotate' ? { mode: 'rotate', axis: active.axis, radians: delta.radians, pivot: active.pivot }
        : { mode: 'scale', axis: active.axis, factor: delta.factor, pivot: active.pivot };
    const commands = insertTransformBatch(current.document, node, edit);
    const outcome = commands ? host.doc!.send(commands as never, `Viewport ${delta.kind}`) as { rejected?: unknown } : null;
    onStatus(!commands ? 'This node cannot be moved that way.' : outcome?.rejected ? 'The document refused that edit.' : 'Committed. Undo with the toolbar.');
    event.preventDefault(); event.stopPropagation();
  };

  // While a drag is on, the box is drawn where it would end up.
  const previewLines: ReactElement[] = [];
  if (preview) {
    const { delta, drag: active } = preview;
    const corners = Array.from({ length: 8 }, (_, c) => [0, 1, 2].map(k => (c & (1 << k) ? active.bounds.max[k] : active.bounds.min[k])));
    const project = (p: number[]) => { const raw = runtime.viewer_project(p[0], p[1], p[2], rect.width, rect.height); return raw ? JSON.parse(raw) as { x: number; y: number } : null; };
    corners.forEach((corner, index) => [1, 2, 4].filter(bit => !(index & bit)).forEach(bit => {
      const a = project(moved(corner, delta, active.axis, active.pivot)), b = project(moved(corners[index ^ bit], delta, active.axis, active.pivot));
      if (a && b) previewLines.push(<line key={`${index}-${bit}`} className="gizmo-preview" x1={a.x} y1={a.y} x2={b.x} y2={b.y} />);
    }));
  }

  return (
    <svg ref={svg} className="gizmo-svg" style={{ left: rect.left, top: rect.top, width: rect.width, height: rect.height }} aria-label="Transform handles"
      onPointerMove={move} onPointerUp={up} onPointerCancel={() => cancel('Drag cancelled.')} onLostPointerCapture={() => cancel('Drag cancelled.')}>
      {mode === 'rotate'
        ? geometry.rings.map((points, axis) => points.length < 3 ? null : (
          <g key={axis}>
            <polyline className="gizmo-hit" points={points.map(p => p.join(',')).join(' ')} onPointerDown={down(axis)} />
            <polyline className={`gizmo-ring axis-${axis}`} points={points.map(p => p.join(',')).join(' ')} />
          </g>))
        : geometry.axes.map(a => (
          <g key={a.axis}>
            <line className="gizmo-hit" x1={a.start[0]} y1={a.start[1]} x2={a.end[0]} y2={a.end[1]} onPointerDown={down(a.axis)} />
            <line className={`gizmo-axis axis-${a.axis} ${mode}`} x1={a.start[0]} y1={a.start[1]} x2={a.end[0]} y2={a.end[1]} />
            {mode === 'scale' && <rect className={`gizmo-cube axis-${a.axis}`} x={a.end[0] - 5} y={a.end[1] - 5} width={10} height={10} />}
          </g>))}
      {previewLines}
    </svg>
  );
}
