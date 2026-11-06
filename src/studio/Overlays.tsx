// View aids drawn over the 3D canvas, like the ones of the plain viewer page and a few more: the box around the selection, its size, the mesh wires
// and normals, boxes round the nodes that have problems, a ground grid, the world axes and a statistics line. They are drawn on a 2D canvas from the
// engine's own projection (`viewer_project`), so they follow the camera exactly, and they never touch the document or what is evaluated.
import { useEffect, useRef, useState } from 'react';
import { host } from '../engine/host';
import { partsFor } from '../../engine/web/panels/graph.js';
import { rt } from './engine-api';
import type { Overlays } from './view';

type Bounds = { min: number[]; max: number[] };
type Point = { x: number; y: number };

const union = (boxes: Bounds[]): Bounds | null => boxes.length
  ? { min: [0, 1, 2].map(k => Math.min(...boxes.map(b => b.min[k]))), max: [0, 1, 2].map(k => Math.max(...boxes.map(b => b.max[k]))) }
  : null;

/** A round step for a grid that suits the model's size: 0.1, 0.2, 0.5, 1, 2, 5, 10 ... metres. */
function niceStep(span: number): number {
  const raw = Math.max(span, 0.5) / 10;
  const magnitude = 10 ** Math.floor(Math.log10(raw));
  const unit = raw / magnitude;
  return magnitude * (unit < 1.5 ? 1 : unit < 3.5 ? 2 : unit < 7.5 ? 5 : 10);
}

export function OverlayCanvas({ overlays, selected, problemNodes }: { overlays: Overlays; selected: string | null; problemNodes: string[] }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const latest = useRef({ overlays, selected, problemNodes });
  latest.current = { overlays, selected, problemNodes };
  const any = overlays.bounds || overlays.dimensions || overlays.wire || overlays.normals || overlays.problems || overlays.grid || overlays.axes;

  useEffect(() => {
    const element = canvas.current;
    if (!element || !any) { element?.getContext('2d')?.clearRect(0, 0, element.width, element.height); return; }
    const context = element.getContext('2d');
    if (!context) return;
    const draw = () => {
      const runtime = rt();
      const evaluation = host.evaluation;
      const rect = host.canvas.getBoundingClientRect();
      if (!runtime || !evaluation || !rect.width || !rect.height) return;
      const dpr = window.devicePixelRatio || 1;
      if (element.width !== Math.round(rect.width * dpr) || element.height !== Math.round(rect.height * dpr)) {
        element.width = Math.round(rect.width * dpr); element.height = Math.round(rect.height * dpr);
      }
      element.style.left = `${rect.left}px`; element.style.top = `${rect.top}px`;
      element.style.width = `${rect.width}px`; element.style.height = `${rect.height}px`;
      context.setTransform(dpr, 0, 0, dpr, 0, 0);
      context.clearRect(0, 0, rect.width, rect.height);
      const { overlays: on, selected: node, problemNodes: bad } = latest.current;
      const project = (p: number[]): Point | null => {
        const raw = runtime.viewer_project(p[0], p[1], p[2], rect.width, rect.height);
        return raw ? JSON.parse(raw) as Point : null;
      };
      const line = (a: number[], b: number[]) => {
        const pa = project(a), pb = project(b);
        if (!pa || !pb) return;
        context.moveTo(pa.x, pa.y); context.lineTo(pb.x, pb.y);
      };
      const box = (b: Bounds) => {
        const c = (i: number) => [i & 1 ? b.max[0] : b.min[0], i & 2 ? b.max[1] : b.min[1], i & 4 ? b.max[2] : b.min[2]];
        for (let i = 0; i < 8; i++) for (const bit of [1, 2, 4]) if (!(i & bit)) line(c(i), c(i | bit));
      };
      const doc = host.doc?.state().document;
      const parts = node && doc ? (partsFor(doc, node) as string[]) : [];
      const selection = union(parts.map(id => evaluation.boundsByNode[id]).filter(Boolean));
      const model = evaluation.bounds;
      const subject = selection ?? model;

      if (on.grid && subject) {
        const step = niceStep(Math.max(subject.max[0] - subject.min[0], subject.max[2] - subject.min[2], 2));
        const cx = Math.round((subject.min[0] + subject.max[0]) / 2 / step) * step, cz = Math.round((subject.min[2] + subject.max[2]) / 2 / step) * step;
        const half = step * 12;
        for (const major of [false, true]) {
          context.beginPath();
          for (let i = -12; i <= 12; i++) {
            if ((i % 5 === 0) !== major) continue;
            line([cx + i * step, 0, cz - half], [cx + i * step, 0, cz + half]);
            line([cx - half, 0, cz + i * step], [cx + half, 0, cz + i * step]);
          }
          context.strokeStyle = major ? 'rgba(255,255,255,0.32)' : 'rgba(255,255,255,0.14)';
          context.lineWidth = 1; context.stroke();
        }
      }
      if (on.axes) {
        const length = subject ? Math.max(0.5, niceStep(Math.max(...subject.max.map((v, k) => v - subject.min[k])))) : 1;
        for (const [axis, colour, label] of [[0, '#ff5d68', 'X'], [1, '#72dc8b', 'Y'], [2, '#62a8ff', 'Z']] as const) {
          const end = [0, 0, 0]; end[axis] = length;
          context.beginPath(); line([0, 0, 0], end); context.strokeStyle = colour; context.lineWidth = 2.5; context.stroke();
          const tip = project(end);
          if (tip) { context.fillStyle = colour; context.font = '600 11px system-ui'; context.fillText(label, tip.x + 5, tip.y - 4); }
        }
      }
      if (on.problems && doc) {
        context.beginPath();
        const seen = new Set<string>();
        for (const id of bad) for (const part of partsFor(doc, id) as string[]) {
          if (seen.has(part)) continue;
          seen.add(part);
          const b = evaluation.boundsByNode[part];
          if (b) box(b);
        }
        context.strokeStyle = '#ff453a'; context.lineWidth = 2; context.stroke();
      }
      if (on.wire || on.normals) {
        const sample = evaluation.overlayForNodes(parts) as { wire: number[][][]; normals: number[][][] };
        const segments = (list: number[][][], colour: string) => {
          context.beginPath();
          const limit = Math.min(list.length, 4000);
          for (let i = 0; i < limit; i++) line(list[i][0], list[i][1]);
          context.strokeStyle = colour; context.lineWidth = 1; context.stroke();
        };
        if (on.wire) {
          // The real triangle edges of the selection (every few triangles of a big mesh, so a frame stays cheap).
          const picked = new Set(parts);
          const meshes = evaluation.parts.filter(part => picked.has(part.node) && !part.transforms?.length);
          const total = meshes.reduce((sum, part) => sum + part.indices.length / 3, 0);
          const stride = Math.max(1, Math.ceil(total / 1500));
          context.beginPath();
          let counted = 0;
          for (const part of meshes) {
            const cache = new Map<number, Point | null>();
            const at = (vertex: number): Point | null => {
              if (!cache.has(vertex)) cache.set(vertex, project([part.positions[vertex * 3], part.positions[vertex * 3 + 1], part.positions[vertex * 3 + 2]]));
              return cache.get(vertex) ?? null;
            };
            for (let t = 0; t < part.indices.length; t += 3) {
              if (counted++ % stride) continue;
              const a = at(part.indices[t]), b = at(part.indices[t + 1]), c = at(part.indices[t + 2]);
              if (!a || !b || !c) continue;
              context.moveTo(a.x, a.y); context.lineTo(b.x, b.y); context.lineTo(c.x, c.y); context.lineTo(a.x, a.y);
            }
          }
          context.strokeStyle = 'rgba(101,240,255,0.65)'; context.lineWidth = 1; context.stroke();
        }
        if (on.normals) segments(sample.normals, 'rgba(255,92,200,0.9)');
      }
      if (on.bounds && subject) {
        context.beginPath(); box(subject);
        context.strokeStyle = selection ? '#ffd166' : 'rgba(255,209,102,0.55)'; context.lineWidth = 1.5; context.setLineDash(selection ? [] : [5, 4]); context.stroke(); context.setLineDash([]);
      }
      if (on.dimensions && subject) {
        const size = subject.max.map((v, k) => v - subject.min[k]);
        const anchor = project([(subject.min[0] + subject.max[0]) / 2, subject.max[1], (subject.min[2] + subject.max[2]) / 2]);
        if (anchor) {
          const text = `${size[0].toFixed(2)} × ${size[1].toFixed(2)} × ${size[2].toFixed(2)} m`;
          context.font = '600 12px ui-monospace, monospace';
          const width = context.measureText(text).width + 16;
          context.fillStyle = 'rgba(20,22,30,0.72)';
          context.beginPath(); context.roundRect(anchor.x - width / 2, anchor.y - 34, width, 22, 11); context.fill();
          context.fillStyle = '#ffd166'; context.fillText(text, anchor.x - width / 2 + 8, anchor.y - 19);
        }
      }
    };
    draw();
    const timer = setInterval(draw, 70);
    return () => clearInterval(timer);
  }, [any]);

  return <canvas ref={canvas} className="overlay-canvas" aria-hidden="true" />;
}

/** Triangles, parts and frames a second, small in a corner of the view. */
export function StatsLine() {
  const [fps, setFps] = useState(0);
  useEffect(() => {
    let frames = 0, last = performance.now(), id = 0;
    const tick = (now: number) => {
      frames++;
      if (now - last >= 500) { setFps(Math.round(frames * 1000 / (now - last))); frames = 0; last = now; }
      id = requestAnimationFrame(tick);
    };
    id = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(id);
  }, []);
  const parts = host.evaluation?.parts ?? [];
  const triangles = parts.reduce((sum, part) => sum + (part.transforms?.length ? (part.indices.length / 3) * (part.transforms.length / 16) : part.indices.length / 3), 0);
  return <div className="statsline glass" role="status">{fps} fps · {parts.length.toLocaleString()} parts · {Math.round(triangles).toLocaleString()} triangles</div>;
}
