import { lazy, Suspense, useCallback, useEffect, useRef, useState } from 'react';
import { Boxes, Download, FolderOpen, LayoutGrid, MessageSquare, Moon, PanelRight, Redo2, Save, SlidersHorizontal, Sun, SunMoon, Undo2, Loader } from 'lucide-react';
import { downloadDocumentText } from './loadWorld';
import { host, mountEngine } from '../engine/host';
import { partsFor } from '../../engine/web/panels/graph.js';
import { useHost, useTheme } from './hooks';
import { buildStage, ChatPanel } from './ChatPanel';
import { Dock, useProblems } from './Dock';
import { Playback } from './Playback';
import { useSelection } from './selection';
import { useTranscript } from './transcript';
import { useWorkspace, workspace } from './library/workspace';
import { useSidebarAgent } from './sidebarAgent';
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
// The start screen carries the bundled example worlds, so it loads only when it is needed.
const ProjectHub = lazy(() => import('./ProjectHub').then(m => ({ default: m.ProjectHub })));
const ExamplesMenu = lazy(() => import('./ExamplesMenu').then(m => ({ default: m.ExamplesMenu })));

type Pane = 'scene' | 'graph';

/** The studio: the engine's canvas full-bleed, with the assistant on the left, the document dock on the right, a graph sheet on demand, and a glass top bar. */
export default function Studio() {
  useHost();
  const pane = useRef<HTMLDivElement>(null);
  const { theme, next } = useTheme();
  const projectState = useWorkspace(host.ready);
  const activeProject = projectState.active;
  const scope = activeProject && projectState.worldId ? { projectId: activeProject.id, worldId: projectState.worldId } : null;
  const { entries, send, clear, log, newChat, selectChat, conversations, conversationId, error: chatError } = useTranscript(scope);
  const agent = useSidebarAgent(scope ? { ...scope, conversationId } : null, log, entries);
  const [projectsOpen, setProjectsOpen] = useState(() => new URLSearchParams(location.search).has('projects'));
  const [sceneVisible, setSceneVisible] = useState(false);
  const sceneLoaded = host.ready && projectState.initialized && !projectState.busy;
  useEffect(() => {
    setSceneVisible(false);
    if (!sceneLoaded) return;
    let second = 0;
    // The renderer applies an accepted projection on its frame loop. Wait for
    // that swap before exposing the canvas, including after project switches.
    const first = requestAnimationFrame(() => { second = requestAnimationFrame(() => setSceneVisible(true)); });
    return () => { cancelAnimationFrame(first); cancelAnimationFrame(second); };
  }, [sceneLoaded, activeProject?.id]);
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

  const [note, setNote] = useState('');
  const [examplesOpen, setExamplesOpen] = useState(false);
  const opener = useRef<HTMLInputElement>(null);
  const [dropping, setDropping] = useState(false);
  useEffect(() => {
    let depth = 0;
    const hasFiles = (e: DragEvent) => !!e.dataTransfer?.types.includes('Files');
    const enter = (e: DragEvent) => { if (hasFiles(e)) { depth++; setDropping(true); e.preventDefault(); } };
    const over = (e: DragEvent) => { if (hasFiles(e)) e.preventDefault(); };
    const leave = (e: DragEvent) => { if (hasFiles(e) && --depth <= 0) { depth = 0; setDropping(false); } };
    const drop = (e: DragEvent) => {
      if (!hasFiles(e)) return;
      e.preventDefault(); depth = 0; setDropping(false);
      const file = e.dataTransfer?.files[0];
      if (file) void workspace.importFile(file).catch(error => setNote(String(error)));
    };
    window.addEventListener('dragenter', enter); window.addEventListener('dragover', over); window.addEventListener('dragleave', leave); window.addEventListener('drop', drop);
    return () => { window.removeEventListener('dragenter', enter); window.removeEventListener('dragover', over); window.removeEventListener('dragleave', leave); window.removeEventListener('drop', drop); };
  }, []);
  useEffect(() => { if (!note) return; const id = setTimeout(() => setNote(''), 5000); return () => clearTimeout(id); }, [note]);

  const state = host.doc?.state();
  const evaluation = host.evaluation?.state;
  const problems = useProblems();
  const { node: selected, select } = useSelection(state?.document);
  const selectedRef = useRef<string | null>(null);
  selectedRef.current = selected;
  const stage = buildStage();

  // A new committed revision becomes an activity card; a failed evaluation becomes a failed one.
  const lastRevision = useRef<string | null>(null);
  const activityWorld = useRef<string | null>(null);
  useEffect(() => {
    if (!state || !activeProject || projectState.busy || state.id !== projectState.documentId) return;
    const revisionKey = state.id + ':' + state.revision;
    if (activityWorld.current !== state.id) { activityWorld.current = state.id; lastRevision.current = revisionKey; return; }
    if (revisionKey === lastRevision.current) return;
    if (lastRevision.current !== null) log({ id: `rev-${state.revision}`, role: 'activity', tool: { name: 'document', status: 'done', summary: `revision ${state.revision} · ${state.nodes} nodes` } });
    lastRevision.current = revisionKey;
  });
  const failure = evaluation?.failureCurrent ? evaluation.lastFailure : null;
  useEffect(() => {
    if (failure && activeProject && !projectState.busy) log({ id: `fail-${failure.code}-${failure.node ?? ''}`, role: 'activity', tool: { name: 'evaluate', status: 'error', summary: failure.code, output: failure.message } });
  }, [failure?.code, failure?.message]);

  const status = host.error ? <span className="chip bad">{host.error}</span>
    : !host.ready ? <span className="chip live"><span className="dot pulse" />Starting engine</span>
      : failure ? <span className="chip bad">{failure.code}</span>
        : stage ? <span className="chip live"><span className="dot pulse" />{stage.label}</span>
          : <span className="chip ok"><span className="dot" />Ready</span>;

  const ThemeIcon = theme === 'auto' ? SunMoon : theme === 'dark' ? Moon : Sun;

  return (
    <div className={`studio${chatOpen ? '' : ' chat-closed'}${dockOpen ? '' : ' dock-closed'}${hidden ? ' hide-chrome' : ''}`}>
      <div className="stage" data-loading={!sceneLoaded || !sceneVisible} ref={pane} />
      {(!sceneLoaded || !sceneVisible) && !host.error && <div className="boot world-loading" role="status"><Loader className="spin" /><span>{host.ready ? 'Opening workspace…' : 'Starting engine…'}</span></div>}

      <header className="topbar glass">
        <a className="brand" href="/" title="Back to the home page"><Boxes size={18} />Cerdic 3D</a>
        <button className="btn" type="button" disabled={agent.running} onClick={() => setProjectsOpen(true)} aria-label="Open projects"><FolderOpen size={15} />{activeProject?.name ?? 'Projects'}</button>
        {activeProject && <span className="project-save-state" role="status">{projectState.busy ? 'Opening…' : projectState.error ? 'Save problem' : projectState.saved ? 'Saved locally' : 'Saving…'}</span>}
        <button className="btn" type="button" data-examples-trigger aria-haspopup="dialog" aria-expanded={examplesOpen} disabled={!host.ready} onClick={() => setExamplesOpen(o => !o)}><LayoutGrid size={15} />Examples</button>
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
          <button className="btn icon" type="button" disabled={!host.ready || !!state?.read_only || !!state?.pending_document} onClick={() => { host.doc?.save(); void workspace.flush().catch(error => setNote(String(error))); }} aria-label="Save" title="Save"><Save size={16} /></button>
          <button className="btn icon" type="button" disabled={!host.ready} onClick={() => opener.current?.click()} aria-label="Open a text file" title="Open a text file"><FolderOpen size={16} /></button>
          <button className="btn icon" type="button" disabled={!host.ready || !state?.nodes} onClick={() => { if (!downloadDocumentText(state?.id ?? 'scene')) setNote('There is nothing to export yet.'); }} aria-label="Export as text" title="Export as text (.txt)"><Download size={16} /></button>
          <button className="btn icon" type="button" aria-pressed={viewOpen} onClick={() => setViewOpen(!viewOpen)} aria-label="View and quality" title="View and quality"><SlidersHorizontal size={16} /></button>
          <button className="btn icon" type="button" onClick={() => setChatOpen(!chatOpen)} aria-label="Toggle assistant" title="Toggle assistant"><MessageSquare size={16} /></button>
          <button className="btn icon" type="button" onClick={() => setDockOpen(!dockOpen)} aria-label="Toggle document panel" title="Toggle document panel"><PanelRight size={16} /></button>
          <button className="btn icon" type="button" onClick={next} aria-label={`Theme: ${theme}`} title={`Theme: ${theme}`}><ThemeIcon size={16} /></button>
        </div>
      </header>

      <input ref={opener} type="file" accept=".txt,.world,text/plain" hidden onChange={e => { const file = e.target.files?.[0]; e.target.value = ''; if (file) void workspace.importFile(file).catch(error => setNote(String(error))); }} />
      {host.ready && projectState.initialized && (projectsOpen || !activeProject) && <Suspense fallback={null}>
        <ProjectHub projects={projectState.projects} active={activeProject} busy={projectState.busy} onClose={() => setProjectsOpen(false)} />
      </Suspense>}
      {examplesOpen && (
        <Suspense fallback={null}>
          <ExamplesMenu hasScene={!!state?.nodes} onOpen={async (text, title, origin) => { await workspace.create(title, text, origin); }} onClose={() => setExamplesOpen(false)} onError={setNote} />
        </Suspense>
      )}
      {dropping && <div className="dropveil" aria-hidden="true">Drop a .txt file to open it</div>}
      {(note || projectState.error) && <div className="lookhint glass" role="alert">{note || projectState.error}</div>}
      <ChatPanel key={`${activeProject?.id ?? 'none'}:${conversationId}`} entries={entries} onSend={text => { send(text); if (agent.status === 'ready') agent.submit(text); }} onClear={clear}
        projectName={activeProject?.name} disabled={!activeProject || projectState.busy || agent.running} conversations={conversations} conversationId={conversationId}
        onNewChat={newChat} onSelectChat={selectChat} error={chatError || agent.error} agent={agent} />
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
