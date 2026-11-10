import { lazy, Suspense, useCallback, useEffect, useRef, useState } from 'react';
import { Boxes, MessageSquare, Moon, PanelRight, Redo2, Save, SlidersHorizontal, Sun, SunMoon, Undo2, Loader } from 'lucide-react';
import { host, mountEngine } from '../engine/host';
import { partsFor } from '../../engine/web/panels/graph.js';
import { useHost, useTheme } from './hooks';
import { buildStage, ChatPanel } from './ChatPanel';
import { Dock, useProblems } from './Dock';
import { Playback } from './Playback';
import { useSelection } from './selection';
import { useTranscript } from './transcript';
import { useViewerControls, useViewSettings } from './view';
import { rt } from './engine-api';
import { ViewPanel } from './ViewPanel';
import { OverlayCanvas, StatsLine } from './Overlays';
import { Gizmo, GizmoBar } from './Gizmo';
import { GameUi } from './GameUi';
import { GameInput } from './GameInput';
import type { Mode, Snap } from './Gizmo';
import './app.css';
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';

const GraphView = lazy(() => import('./GraphView').then(m => ({ default: m.GraphView })));

type Pane = 'scene' | 'graph';

/** The studio: the engine's canvas full-bleed, with the assistant on the left, the document dock on the right, a graph sheet on demand, and a glass top bar. */
export default function Studio() {
  useHost();
  const pane = useRef<HTMLDivElement>(null);
  const { theme, next } = useTheme();
  const { entries, send, clear, log } = useTranscript();
  const [mode, setMode] = useState<Pane>('scene');
  const [chatOpen, setChatOpen] = useState(true);
  const [dockOpen, setDockOpen] = useState(true);
  const [viewOpen, setViewOpen] = useState(false);
  const [hidden, setHidden] = useState(false);
  const { view, set } = useViewSettings();

  useEffect(() => { if (pane.current) void mountEngine(pane.current); }, []);

  // F frames the selected node's parts, or the scene's subject (the model without its sea, terrain or ground). R goes back to that home view of the whole scene.
  const frameBounds = useCallback((selected: boolean) => {
    const evaluation = host.evaluation;
    const runtime = rt();
    if (!evaluation || !runtime) return;
    const doc = host.doc?.state().document;
    const ids = selected && selectedRef.current && doc ? (partsFor(doc, selectedRef.current) as string[]) : [];
    const boxes = ids.map(id => evaluation.boundsByNode[id]).filter(Boolean);
    const bounds = boxes.length
      ? { min: [0, 1, 2].map(k => Math.min(...boxes.map(b => b.min[k]))), max: [0, 1, 2].map(k => Math.max(...boxes.map(b => b.max[k]))) }
      : evaluation.subject ?? evaluation.bounds;
    if (bounds) runtime.viewer_frame(bounds.min[0], bounds.min[1], bounds.min[2], bounds.max[0], bounds.max[1], bounds.max[2]);
  }, []);
  const frame = useCallback(() => frameBounds(true), [frameBounds]);
  const home = useCallback(() => frameBounds(false), [frameBounds]);
  const toggleChrome = useCallback(() => setHidden(h => !h), []);
  useViewerControls(view, set, frame, toggleChrome, home);
  const [tool, setTool] = useState<Mode>('translate');
  const [snap, setSnap] = useState<Snap>({ grid: 0.1, angle: 15, scale: 0.1 });
  const [toolStatus, setToolStatus] = useState('');
  const [locked, setLocked] = useState(false);
  useEffect(() => {
    const change = () => setLocked(document.pointerLockElement === host.canvas);
    document.addEventListener('pointerlockchange', change);
    return () => document.removeEventListener('pointerlockchange', change);
  }, []);

  // ?world=worlds/name.txt loads a program in the text language (from the engine bundle's worlds folder) as the document, once the evaluator is idle.
  useEffect(() => {
    const world = new URLSearchParams(location.search).get('world');
    if (!world) return;
    let live = true;
    const url = /^(https?:)?\//.test(world) ? world : `/engine/${world}`;
    void (async () => {
      const text = await fetch(url).then(r => r.text());
      while (live && !(host.ready && host.evaluation?.state.worker === 'idle')) await new Promise(r => setTimeout(r, 150));
      if (!live) return;
      // The engine keeps the last document in the browser, so a world is loaded into a fresh one: otherwise it is added to whatever was there
      // (the tractor would still be in the scene when the whale loads).
      host.runtime?.doc_reset(new URLSearchParams(location.search).get('docname') ?? 'spatial-lab', 1);
      host.evaluation?.request();
      host.doc?.sendText(text, 'world');
    })();
    return () => { live = false; };
  }, []);

  const state = host.doc?.state();
  const evaluation = host.evaluation?.state;
  const problems = useProblems();
  const { node: selected, select } = useSelection(state?.document);
  const selectedRef = useRef<string | null>(null);
  selectedRef.current = selected;
  const stage = buildStage();

  // A new committed revision becomes an activity card; a failed evaluation becomes a failed one.
  const lastRevision = useRef<number | null>(null);
  useEffect(() => {
    if (!state || state.revision === lastRevision.current) return;
    if (lastRevision.current !== null) log({ id: `rev-${state.revision}`, role: 'activity', tool: { name: 'document', status: 'done', summary: `revision ${state.revision} · ${state.nodes} nodes` } });
    lastRevision.current = state.revision;
  });
  const failure = evaluation?.failureCurrent ? evaluation.lastFailure : null;
  useEffect(() => {
    if (failure) log({ id: `fail-${failure.code}-${failure.node ?? ''}`, role: 'activity', tool: { name: 'evaluate', status: 'error', summary: failure.code, output: failure.message } });
  }, [failure?.code, failure?.message]);

  const status = host.error ? <span className="chip bad">{host.error}</span>
    : !host.ready ? <span className="chip live"><span className="dot pulse" />Starting engine</span>
      : failure ? <span className="chip bad">{failure.code}</span>
        : stage ? <span className="chip live"><span className="dot pulse" />{stage.label}</span>
          : <span className="chip ok"><span className="dot" />Ready</span>;

  const ThemeIcon = theme === 'auto' ? SunMoon : theme === 'dark' ? Moon : Sun;

  return (
    <div className={`studio${chatOpen ? '' : ' chat-closed'}${dockOpen ? '' : ' dock-closed'}${hidden ? ' hide-chrome' : ''}`}>
      <div className="stage" ref={pane} />
      {!host.ready && !host.error && <div className="boot"><Loader className="spin" /></div>}

      <header className="topbar glass">
        <div className="brand"><Boxes size={18} />Spatial Lab</div>
        <span className="docname"><b>{state?.id ?? 'Untitled'}</b>{state ? ` · revision ${state.revision}` : ''}</span>
        <div className="grow" />
        <div className="segmented" role="group" aria-label="View">
          <button type="button" aria-pressed={mode === 'scene'} onClick={() => setMode('scene')}>Scene</button>
          <button type="button" aria-pressed={mode === 'graph'} onClick={() => setMode('graph')}>Graph</button>
        </div>
        <div className="grow" />
        {status}
        <div className="group">
          <button className="btn icon" type="button" disabled={!state?.can_undo} onClick={() => host.doc?.undo()} aria-label="Undo" title="Undo"><Undo2 size={16} /></button>
          <button className="btn icon" type="button" disabled={!state?.can_redo} onClick={() => host.doc?.redo()} aria-label="Redo" title="Redo"><Redo2 size={16} /></button>
          <button className="btn icon" type="button" disabled={!host.ready || !!state?.read_only || !!state?.pending_document} onClick={() => host.doc?.save()} aria-label="Save" title="Save"><Save size={16} /></button>
          <button className="btn icon" type="button" aria-pressed={viewOpen} onClick={() => setViewOpen(!viewOpen)} aria-label="View and quality" title="View and quality"><SlidersHorizontal size={16} /></button>
          <button className="btn icon" type="button" onClick={() => setChatOpen(!chatOpen)} aria-label="Toggle assistant" title="Toggle assistant"><MessageSquare size={16} /></button>
          <button className="btn icon" type="button" onClick={() => setDockOpen(!dockOpen)} aria-label="Toggle document panel" title="Toggle document panel"><PanelRight size={16} /></button>
          <button className="btn icon" type="button" onClick={next} aria-label={`Theme: ${theme}`} title={`Theme: ${theme}`}><ThemeIcon size={16} /></button>
        </div>
      </header>

      <ChatPanel entries={entries} onSend={send} onClear={clear} />
      <Dock doc={state?.document} selected={selected} onSelect={select} problems={problems} />
      <Playback />
      {host.ready && <GameUi />}
      {host.ready && <GameInput />}
      <OverlayCanvas overlays={view.overlays} selected={selected} problemNodes={problems.flatMap(p => [p.node, ...(p.related ?? [])]).filter((id): id is string => !!id)} />
      {view.overlays.stats && <StatsLine />}
      {selected && host.ready && <>
        <Gizmo node={selected} mode={tool} snap={snap} onStatus={setToolStatus} />
        <GizmoBar mode={tool} setMode={setTool} snap={snap} setSnap={setSnap} status={toolStatus} />
      </>}
      {host.ready && !locked && <div className="lookhint glass" role="note">Click the view to look around · WASD to move · Esc to let go</div>}
      {viewOpen && <ViewPanel view={view} set={set} onFrame={frame} />}

      {mode === 'graph' && state && (
        <section className="sheet glass strong" aria-label="Node graph">
          <Suspense fallback={<div className="graph-msg"><Loader className="spin" /></div>}>
            <GraphView doc={state.document} revision={state.revision} />
          </Suspense>
        </section>
      )}
      {host.error && <div className="boot error">{host.error}</div>}
    </div>
  );
}
