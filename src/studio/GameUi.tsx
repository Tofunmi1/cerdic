// The game's own UI (health bars, text, panels set by game scripts) drawn over the 3D view. The engine lays it out (units are fractions of the
// screen height, text is procedural skeleton-font strokes, no font files) and this draws the rectangles and pen strokes it returns; it never
// decides what is shown. It is a thin renderer: the same frame could be drawn by the engine itself later.
import { useEffect, useRef } from 'react';
import { host } from '../engine/host';
import { rt } from './engine-api';
import type { ButtonBox } from './actionMap';
import { uiButtons } from './uiButtons';

type Item =
  | { k: 'rect'; x: number; y: number; w: number; h: number; fill: string; radius: number; stroke: string | null; stroke_w: number }
  | { k: 'path'; pts: number[]; w: number; color: string; round: boolean };

export function GameUi() {
  const canvas = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const element = canvas.current;
    const context = element?.getContext('2d');
    if (!element || !context) return;
    let frame = 0;
    let lastText = '';
    const draw = () => {
      frame = requestAnimationFrame(draw);
      const runtime = rt() as unknown as { game_ui_frame?(aspect: number): string } | null;
      const rect = host.canvas.getBoundingClientRect();
      if (!runtime?.game_ui_frame || !rect.width || !rect.height) return;
      const dpr = window.devicePixelRatio || 1;
      const width = Math.round(rect.width * dpr), height = Math.round(rect.height * dpr);
      if (element.width !== width || element.height !== height) { element.width = width; element.height = height; lastText = ''; }
      element.style.left = `${rect.left}px`; element.style.top = `${rect.top}px`;
      element.style.width = `${rect.width}px`; element.style.height = `${rect.height}px`;
      const text = runtime.game_ui_frame(rect.width / rect.height);
      if (text === lastText) return;
      lastText = text;
      const parsed = JSON.parse(text) as { items: Item[]; buttons?: ButtonBox[] };
      uiButtons.current = parsed.buttons ?? [];
      context.setTransform(1, 0, 0, 1, 0, 0);
      context.clearRect(0, 0, width, height);
      if (!parsed.items.length) return;
      const unit = height; // one screen height in pixels
      for (const item of parsed.items) {
        if (item.k === 'rect') {
          context.beginPath();
          context.roundRect(item.x * unit, item.y * unit, item.w * unit, item.h * unit, Math.min(item.radius * unit, item.h * unit / 2, item.w * unit / 2));
          context.fillStyle = item.fill; context.fill();
          if (item.stroke) { context.strokeStyle = item.stroke; context.lineWidth = Math.max(1, item.stroke_w * unit); context.stroke(); }
        } else {
          context.beginPath();
          for (let i = 0; i < item.pts.length; i += 2) {
            const x = item.pts[i] * unit, y = item.pts[i + 1] * unit;
            if (i === 0) context.moveTo(x, y); else context.lineTo(x, y);
          }
          if (item.pts.length === 4 && item.pts[0] === item.pts[2] && item.pts[1] === item.pts[3]) context.lineTo(item.pts[0] * unit + 0.01, item.pts[1] * unit);
          context.strokeStyle = item.color; context.lineWidth = Math.max(1, item.w * unit);
          context.lineCap = item.round ? 'round' : 'butt'; context.lineJoin = item.round ? 'round' : 'miter';
          context.stroke();
        }
      }
    };
    frame = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(frame);
  }, []);

  return <canvas ref={canvas} className="overlay-canvas game-ui" aria-hidden="true" />;
}
