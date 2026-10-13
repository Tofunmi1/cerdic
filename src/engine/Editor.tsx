import { useEffect, useReducer, useRef, useState } from 'react';
import type { Command } from './generated';
import { host, mountEngine } from './host';

export default function Editor() {
  const pane = useRef<HTMLDivElement>(null);
  const [, refresh] = useReducer(n => n + 1, 0);
  const [command, setCommand] = useState('[]');
  const [notice, setNotice] = useState('');
  const [width, setWidth] = useState(280);
  useEffect(() => {
    const unsubscribe = host.subscribe(refresh);
    if (pane.current) void mountEngine(pane.current);
    return unsubscribe;
  }, []);
  const state = host.doc?.state();
  const evaluation = host.evaluation?.state;
  const action = (run: () => unknown) => {
    try { setNotice(JSON.stringify(run(), null, 2)); }
    catch (error) { setNotice(String(error)); }
  };
  const addBox = () => host.doc?.send([{ add_node: { op: 'solid.box@1', params: { size: [1, 1, 1] },
    label: 'Box', alias: '$box', group: null } }], 'Add box');
  return <main className="engine-editor" style={{ gridTemplateColumns: `${width}px minmax(0,1fr)` }}>
    <header><strong>Spatial Lab</strong><span>Procedural workspace</span>
      <button disabled={!host.xr} onClick={() => action(() => host.runtime?.enter_xr('ve').catch(e => setNotice(String(e))))}>
        {host.xr ? 'Enter VR' : 'VR unavailable'}</button></header>
    <aside>
      <h2>Document</h2><p>{state?.id ?? 'Starting engine…'} · revision {state?.revision ?? 0}</p>
      <div className="engine-actions">
        <button disabled={!host.ready || state?.read_only} onClick={() => action(addBox)}>Add box</button>
        <button disabled={!state?.can_undo} onClick={() => action(() => host.doc?.undo())}>Undo</button>
        <button disabled={!state?.can_redo} onClick={() => action(() => host.doc?.redo())}>Redo</button>
        <button disabled={!host.ready || state?.read_only || !!state?.pending_document} onClick={() => action(() => host.doc?.save())}>Save</button>
      </div>
      <label htmlFor="commands">Command JSON</label>
      <textarea id="commands" value={command} onChange={e => setCommand(e.target.value)} spellCheck={false} />
      <button disabled={!host.ready} onClick={() => action(() => host.doc?.send(JSON.parse(command) as Command[], 'Manual batch'))}>Send commands</button>
      <label htmlFor="panel-width">Panel width</label>
      <input id="panel-width" type="range" min="220" max="460" value={width} onChange={e => setWidth(Number(e.target.value))} />
      <h2>Evaluation</h2>
      <pre data-testid="evaluation-status" className="engine-status">{JSON.stringify(evaluation ?? {}, null, 2)}</pre>
      <h2>Live document</h2>
      {state?.pending_document && <p>Pending revision {state.pending_document.revision} · evaluating before commit</p>}
      <pre data-testid="document-view">{JSON.stringify(state?.document ?? {}, null, 2)}</pre>
    </aside>
    <section className="engine-stage" aria-label="Scene"><div ref={pane} className="engine-canvas-pane" />
      <div className="engine-hint">Click to focus · WASD move · Q/E vertical · double-click to look · Esc release</div>
    </section>
    <footer role="status">{host.error || notice || (host.ready ? `Engine ready · worker ${evaluation?.worker ?? 'starting'}` : 'Starting Rust engine…')}</footer>
  </main>;
}
