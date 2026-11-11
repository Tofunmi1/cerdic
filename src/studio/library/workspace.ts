import { useEffect, useSyncExternalStore } from 'react';
import { host } from '../../engine/host';
import { loadWorldText } from '../loadWorld';
import { byRecent, createProject, idbProjectStore, renameProject } from './projects';
import type { Project } from './projects';
import { checkpointWorld, projectWorldId, recoverCheckpoint } from './checkpoints';

type Snapshot = { projects: Project[]; active: Project | null; documentId: string | null; agentBusy: boolean; busy: boolean; initialized: boolean; saved: boolean; error: string };
const store = idbProjectStore();
let state: Snapshot = { projects: [], active: null, documentId: null, agentBusy: false, busy: false, initialized: false, saved: true, error: '' };
const listeners = new Set<() => void>();
const update = (patch: Partial<Snapshot>) => { state = { ...state, ...patch }; listeners.forEach(fn => fn()); };
let initialization: Promise<void> | undefined;
let timer: ReturnType<typeof setTimeout> | undefined;
let observedRevision = '';
let writes = Promise.resolve();
const write = (project: Project) => {
  const task = writes.then(() => store.put(project));
  writes = task.catch(() => {});
  return task;
};
const worldId = projectWorldId;
const route = (project: Project) => {
  const url = new URL(location.href);
  ['world', 'example', 'docname', 'projects'].forEach(key => url.searchParams.delete(key));
  url.searchParams.set('project', project.id);
  history.replaceState(null, '', url);
};
async function flush() {
  clearTimeout(timer);
  const active = state.active;
  if (!active || state.busy || state.saved) return;
  const view = host.doc?.state();
  if (!view || view.id !== state.documentId || view.pending_document) return;
  const text = host.doc?.text().text;
  if (text === undefined || (text === active.text && state.saved)) return;
  const project = { ...active, text, updatedAt: Date.now() };
  const savedDocumentId = view.id, savedRevision = view.revision;
  update({ saved: false });
  try {
    try { checkpointWorld(localStorage, active, text, view.revision, project.updatedAt); } catch { /* IndexedDB save below is still attempted. */ }
    await write(project);
    const current = host.doc?.state();
    const caughtUp = current?.id === savedDocumentId && current.revision === savedRevision && !current.pending_document;
    update({ projects: byRecent(state.projects.map(p => p.id === project.id ? project : p)),
      ...(state.active?.id === project.id ? { active: project, saved: caughtUp } : {}), error: '' });
  } catch (error) { update({ error: `Could not save project: ${String(error)}`, saved: false }); throw error; }
}
async function open(project: Project) {
  if (state.agentBusy) throw new Error('Stop the assistant before switching projects.');
  if (state.busy) throw new Error('Wait for the current project to finish opening.');
  await flush();
  if (state.active && !state.saved) throw new Error('The world changed while saving. Wait for it to save before switching projects.');
  project = state.projects.find(p => p.id === project.id) ?? project;
  if (host.doc?.state().pending_document || host.evaluation?.state.worker !== 'idle') throw new Error('Wait for the current edit to finish before switching projects.');
  const previous = state.active;
  update({ busy: true, error: '' });
  try {
    const recovered = recoverCheckpoint(localStorage, project);
    if (recovered !== project) {
      await write(recovered);
      project = recovered;
      update({ projects: byRecent(state.projects.map(p => p.id === project.id ? project : p)) });
    }
    await loadWorldText(project.text, worldId(project));
    const deadline = Date.now() + 120000;
    while (host.doc?.state().pending_document || host.evaluation?.state.worker !== 'idle') {
      if (Date.now() > deadline) throw new Error('The world took too long to open.');
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    if (host.evaluation?.state.failureCurrent) throw new Error(host.evaluation.state.lastFailure?.message ?? 'World evaluation failed.');
    const bounds = host.evaluation?.subject ?? host.evaluation?.bounds;
    if (bounds) host.runtime?.viewer_frame(bounds.min[0], bounds.min[1], bounds.min[2], bounds.max[0], bounds.max[1], bounds.max[2]);
    route(project);
    observedRevision = `${host.doc?.state().id}:${host.doc?.state().revision}`;
    update({ active: project, documentId: host.doc?.state().id ?? null, saved: true });
  } catch (error) {
    if (previous) await loadWorldText(previous.text, worldId(previous));
    update({ error: String(error) });
    throw error;
  } finally { update({ busy: false }); }
}
async function create(name: string, text = '', origin = 'blank') {
  if (state.agentBusy) throw new Error('Stop the assistant before creating a project.');
  if (state.busy) throw new Error('Wait for the current project to finish opening.');
  const project = createProject(state.projects, text, name, origin);
  await write(project);
  update({ projects: byRecent([...state.projects, project]) });
  try { await open(project); }
  catch (error) {
    await store.remove(project.id);
    update({ projects: state.projects.filter(p => p.id !== project.id) });
    throw error;
  }
  return project;
}
async function initialize() {
  try {
    const deadline = Date.now() + 120000;
    while (host.evaluation?.state.worker !== 'idle' || host.doc?.state().pending_document) {
      if (Date.now() > deadline) throw new Error('The engine did not finish restoring its document. Reload to try again.');
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    update({ projects: byRecent(await store.list()) });
    const params = new URLSearchParams(location.search);
    const id = params.get('project');
    const world = params.get('world');
    const example = params.get('example');
    if (id) {
      const project = state.projects.find(p => p.id === id);
      if (!project) throw new Error('This project is not saved in this browser. Open Projects to choose or create one.');
      await open(project);
    } else if (world) {
      const response = await fetch(/^(https?:)?\//.test(world) ? world : `/engine/${world}`);
      if (!response.ok) throw new Error(`Could not load scene (${response.status})`);
      await create(params.get('docname') ?? world.split('/').pop()?.replace(/\.txt$/, '') ?? 'Imported world', await response.text(), world);
    } else if (example) {
      const { EXAMPLES } = await import('../examples');
      const source = EXAMPLES.find(item => item.id === example);
      if (!source) throw new Error(`Unknown example: ${example}`);
      await create(source.title, source.text, example);
    } else if (!state.projects.length && host.doc?.state().nodes) {
      await create('Recovered world', host.doc.text().text ?? '', 'recovered');
    }
  } catch (error) { update({ error: String(error) }); }
  finally { update({ initialized: true }); }
}
host.subscribe(() => {
  const view = host.doc?.state();
  if (!state.initialized || !state.active || state.busy || !view || view.pending_document || view.id !== state.documentId) return;
  const revision = `${view.id}:${view.revision}`;
  if (revision === observedRevision) return;
  observedRevision = revision;
  clearTimeout(timer);
  update({ saved: false });
  // Capture committed text synchronously, before the debounced IndexedDB write.
  // pagehide cannot reliably finish an asynchronous save during reload.
  let delay = 600;
  const text = host.doc?.text().text;
  if (text !== undefined) {
    try { checkpointWorld(localStorage, state.active, text, view.revision); }
    catch { delay = 0; update({ error: 'Recovery checkpoint unavailable. Saving directly to project storage…' }); }
  }
  timer = setTimeout(() => { void flush().catch(() => {}); }, delay);
});
window.addEventListener('pagehide', () => { void flush().catch(() => {}); });

export const workspace = {
  setAgentBusy(agentBusy: boolean) { update({ agentBusy }); },
  create, open, flush,
  async rename(project: Project, name: string) {
    await flush();
    const current = state.projects.find(p => p.id === project.id) ?? project;
    const renamed = renameProject(state.projects, current, name);
    await write(renamed);
    update({ projects: byRecent(state.projects.map(p => p.id === renamed.id ? renamed : p)), ...(state.active?.id === renamed.id ? { active: renamed } : {}) });
  },
  async importFile(file: File) {
    if (!/\.(txt|world|ve)$/i.test(file.name) && !file.type.startsWith('text/')) throw new Error('Choose a scene program (.txt, .world or .ve).');
    return create(file.name.replace(/\.[^.]+$/, ''), await file.text(), 'file');
  },
};
export function useWorkspace(ready: boolean) {
  const snapshot = useSyncExternalStore(fn => { listeners.add(fn); return () => { listeners.delete(fn); }; }, () => state);
  useEffect(() => { if (ready) initialization ??= initialize(); }, [ready]);
  return { ...snapshot, worldId: snapshot.active ? worldId(snapshot.active) : null };
}

// A module reload must not leave an old subscription saving a replacement
// engine's temporary startup document into an active project.
if (import.meta.hot) import.meta.hot.accept(() => location.reload());
