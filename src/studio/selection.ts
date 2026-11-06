// The selected node, shared between the panels and the viewport. Selecting in a panel tells the engine which parts of the model to light up
// (the ones the node builds); clicking a part in the viewport comes back here. Selection is view state: never saved, never a command.
import { useCallback, useEffect, useRef, useState } from 'react';
import type { Document } from '../engine/generated';
import { partsFor } from '../../engine/web/panels/graph.js';
import { rt } from './engine-api';

export function useSelection(doc: Document | undefined) {
  const [node, setNode] = useState<string | null>(null);
  const current = useRef<string | null>(null);
  const seen = useRef(-1);
  const docRef = useRef(doc);
  docRef.current = doc;

  const look = () => { const runtime = rt(); return runtime ? JSON.parse(runtime.selection_get()) as { node: string | null; generation: number } : null; };

  const push = useCallback((id: string | null) => {
    const runtime = rt();
    if (!runtime) return;
    if (id && docRef.current) runtime.selection_set(id, JSON.stringify(partsFor(docRef.current, id)));
    else runtime.selection_clear();
    seen.current = look()?.generation ?? seen.current;
  }, []);

  const select = useCallback((id: string | null) => {
    current.current = id; setNode(id); push(id);
  }, [push]);

  // A click on a part in the viewport changes the engine's selection behind our back.
  useEffect(() => {
    const timer = setInterval(() => {
      const engine = look();
      if (!engine || engine.generation === seen.current) return;
      seen.current = engine.generation;
      if (engine.node !== current.current) { current.current = engine.node; setNode(engine.node); }
    }, 100);
    return () => clearInterval(timer);
  }, []);

  // After the document changed: forget a node that no longer exists, and aim at the parts it builds now.
  const revision = doc?.revision;
  useEffect(() => {
    if (!doc || !current.current) return;
    const id = current.current;
    const exists = doc.nodes.some(n => n.id === id) || doc.groups?.some(g => g.id === id);
    if (!exists) { current.current = null; setNode(null); push(null); } else push(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [revision]);

  return { node, select };
}
