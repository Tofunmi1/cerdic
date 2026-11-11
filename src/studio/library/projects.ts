// The project library: every scene the person has made, kept in the browser. A project is the program text plus a little about it (name, when it changed,
// a small picture). The store is injected, like the game save slots: IndexedDB in the page (`idbProjectStore`), a map in tests (`memoryProjectStore`).
// Nothing here talks to a server.

export type Project = {
  id: string;
  name: string;
  /** Stable identity for the project's initial world, independent of its display name. */
  worldId?: string;
  /** The program in the scene language. */
  text: string;
  createdAt: number;
  updatedAt: number;
  /** A small JPEG as a data URL, shown on the project card; empty until one has been taken. */
  thumb?: string;
  /** Where it came from: an example's id, "file" for an opened file, "assistant" for one the model built, "blank" for a new one. */
  origin?: string;
};

export interface ProjectStore {
  list(): Promise<Project[]>;
  get(id: string): Promise<Project | undefined>;
  put(project: Project): Promise<void>;
  remove(id: string): Promise<void>;
}

/** A short random id that is safe in a URL and a file name. */
export const newProjectId = () => `p-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`;

/** `base`, or `base 2`, `base 3` … the first name no other project has. */
export function uniqueName(existing: readonly string[], base: string): string {
  const taken = new Set(existing.map((name) => name.toLowerCase()));
  const clean = base.trim() || 'Untitled';
  if (!taken.has(clean.toLowerCase())) return clean;
  for (let n = 2; ; n++) if (!taken.has(`${clean} ${n}`.toLowerCase())) return `${clean} ${n}`;
}

/** The projects, newest change first. */
export const byRecent = (projects: readonly Project[]) => [...projects].sort((a, b) => b.updatedAt - a.updatedAt);

/** A new project from text, named so it does not clash with the existing ones. */
export function createProject(existing: readonly Project[], text: string, name: string, origin?: string, now = Date.now()): Project {
  return { id: newProjectId(), worldId: crypto.randomUUID(), name: uniqueName(existing.map((p) => p.name), name), text, createdAt: now, updatedAt: now, origin };
}

/** A copy of a project under a new id and name. */
export function duplicateProject(existing: readonly Project[], source: Project, now = Date.now()): Project {
  return { ...source, id: newProjectId(), worldId: crypto.randomUUID(), name: uniqueName(existing.map((p) => p.name), `${source.name} copy`), createdAt: now, updatedAt: now };
}

/** A project with its name changed, refusing an empty one. */
export function renameProject(existing: readonly Project[], project: Project, name: string, now = Date.now()): Project {
  const wanted = name.trim();
  if (!wanted || wanted === project.name) return project;
  const others = existing.filter((p) => p.id !== project.id).map((p) => p.name);
  return { ...project, name: uniqueName(others, wanted), updatedAt: now };
}

/** The whole library as one JSON document the person can keep or move to another browser, and the way back in. */
export function exportLibrary(projects: readonly Project[]): string {
  return JSON.stringify({ format: 'cerdic-library', version: 1, projects }, null, 1);
}
export function parseLibrary(json: string): Project[] {
  const data = JSON.parse(json) as { format?: string; projects?: unknown };
  if (data.format !== 'cerdic-library' || !Array.isArray(data.projects)) throw new Error('That is not a Cerdic library file.');
  return data.projects.filter((p): p is Project => !!p && typeof p === 'object' && typeof (p as Project).id === 'string' && typeof (p as Project).text === 'string' && typeof (p as Project).name === 'string');
}

/** Projects in memory: for tests, and the fallback when the browser has no storage. */
export function memoryProjectStore(initial: readonly Project[] = []): ProjectStore {
  const map = new Map(initial.map((p) => [p.id, p]));
  return {
    list: async () => [...map.values()],
    get: async (id) => map.get(id),
    put: async (project) => { map.set(project.id, project); },
    remove: async (id) => { map.delete(id); },
  };
}

const DATABASE = 'cerdic-library';
const STORE = 'projects';

/** Projects in the browser's IndexedDB. */
export function idbProjectStore(): ProjectStore {
  let opened: Promise<IDBDatabase> | null = null;
  const open = () => (opened ??= new Promise<IDBDatabase>((resolve, reject) => {
    const request = indexedDB.open(DATABASE, 1);
    request.onupgradeneeded = () => { request.result.createObjectStore(STORE, { keyPath: 'id' }); };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error('IndexedDB is not available'));
  }));
  const run = async <T>(mode: IDBTransactionMode, work: (store: IDBObjectStore) => IDBRequest<T>): Promise<T> => {
    const db = await open();
    return new Promise<T>((resolve, reject) => {
      const transaction = db.transaction(STORE, mode);
      const request = work(transaction.objectStore(STORE));
      transaction.oncomplete = () => resolve(request.result);
      transaction.onabort = () => reject(transaction.error ?? new Error('Project storage transaction aborted'));
      request.onerror = () => reject(request.error ?? new Error('IndexedDB failed'));
    });
  };
  return {
    list: () => run<Project[]>('readonly', (store) => store.getAll()),
    get: (id) => run<Project | undefined>('readonly', (store) => store.get(id)),
    put: async (project) => { await run('readwrite', (store) => store.put(project)); },
    remove: async (id) => { await run('readwrite', (store) => store.delete(id)); },
  };
}
