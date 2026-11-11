import { useEffect, useRef, useState } from 'react';
import './landing-extra.css';

// Numbers that are true of the repository: the operator count and families come from the registry (src/engine/operators.d.ts), the texture library from the
// Material Maker catalogue, the polygon-operator count from the registry's `poly` family, the crate count from the workspace.
const STATS = [
  { value: 263, label: 'operators', sub: 'in 38 families' },
  { value: 398, label: 'texture nodes', sub: 'baked in the browser' },
  { value: 60, label: 'polygon-editing operators', sub: 'bevel, inset, extrude, unwrap' },
  { value: 29, label: 'Rust crates', sub: 'no engine code in the core' },
  { value: 0, label: 'imported models', sub: 'all of it is generated' },
];

/** Counts from 0 to `target` once `start` is true; shows the final number straight away for a visitor who asked for less motion. */
function useCount(target: number, start: boolean, ms = 1300) {
  const [value, setValue] = useState(() => (typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches ? target : 0));
  useEffect(() => {
    if (!start || value === target) return;
    if (matchMedia('(prefers-reduced-motion: reduce)').matches) { setValue(target); return; }
    let frame = 0;
    const began = performance.now();
    const tick = (now: number) => {
      const t = Math.min(1, (now - began) / ms);
      setValue(Math.round(target * (1 - Math.pow(1 - t, 3))));
      if (t < 1) frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [start, target, ms]); // eslint-disable-line react-hooks/exhaustive-deps
  return value;
}

function Stat({ value, prefix = '', label, sub, start }: { value: number; prefix?: string; label: string; sub: string; start: boolean }) {
  const shown = useCount(value, start);
  return (
    <div className="stat">
      <span className="stat-n" aria-label={`${prefix}${value}`}>{prefix}{shown}</span>
      <span className="stat-label">{label}</span>
      <span className="stat-sub">{sub}</span>
    </div>
  );
}

/** A band of numbers that count up as it scrolls into view. */
export function Stats() {
  const root = useRef<HTMLDivElement>(null);
  const [start, setStart] = useState(false);
  useEffect(() => {
    const node = root.current;
    if (!node) return;
    const watch = new IntersectionObserver(([entry]) => { if (entry.isIntersecting) { setStart(true); watch.disconnect(); } }, { threshold: 0.4 });
    watch.observe(node);
    return () => watch.disconnect();
  }, []);
  return (
    <div className="stats" ref={root} data-reveal role="group" aria-label="The engine in numbers">
      {STATS.map((stat) => <Stat key={stat.label} {...stat} start={start} />)}
    </div>
  );
}
