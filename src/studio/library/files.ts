// Files the person keeps in the browser: exported models, snapshots, imported text. They go in the origin private file system when the browser has one (fast,
// large, real files) and in IndexedDB when it does not. As with the projects, the store is injected so tests need no browser.

export type StoredFile = { path: string; size: number; type: string; savedAt: number };

export interface FileStore {
  put(path: string, data: Blob): Promise<StoredFile>;
  get(path: string): Promise<Blob | undefined>;
  list(prefix?: string): Promise<StoredFile[]>;
  remove(path: string): Promise<void>;
}

/** A path with no `..`, no leading slash and no odd characters, so it is safe as a file name in any of the stores. */
export function cleanPath(raw: string): string {
  const parts = raw.split('/').map((part) => part.trim().replace(/[^\w.\- ]+/g, '-')).filter((part) => part && part !== '.' && part !== '..');
  if (parts.length === 0) throw new Error('A file needs a name.');
  return parts.join('/').slice(0, 200);
}

/** `name.ext`, or `name 2.ext`, `name 3.ext` … the first that is not taken in `existing`. */
export function freshPath(existing: readonly string[], wanted: string): string {
  const taken = new Set(existing);
  const path = cleanPath(wanted);
  if (!taken.has(path)) return path;
  const dot = path.lastIndexOf('.');
  const [stem, ext] = dot > path.lastIndexOf('/') && dot > 0 ? [path.slice(0, dot), path.slice(dot)] : [path, ''];
  for (let n = 2; ; n++) if (!taken.has(`${stem} ${n}${ext}`)) return `${stem} ${n}${ext}`;
}

export const humanSize = (bytes: number) => bytes < 1024 ? `${bytes} B` : bytes < 1024 * 1024 ? `${(bytes / 1024).toFixed(1)} KB` : `${(bytes / 1024 / 1024).toFixed(1)} MB`;

/** Files in memory: for tests, and the last resort when the browser has no storage. */
export function memoryFileStore(now: () => number = Date.now): FileStore {
  const files = new Map<string, { data: Blob; savedAt: number }>();
  const info = (path: string, entry: { data: Blob; savedAt: number }): StoredFile => ({ path, size: entry.data.size, type: entry.data.type, savedAt: entry.savedAt });
  return {
    put: async (path, data) => { const key = cleanPath(path); const entry = { data, savedAt: now() }; files.set(key, entry); return info(key, entry); },
    get: async (path) => files.get(cleanPath(path))?.data,
    list: async (prefix = '') => [...files].filter(([path]) => path.startsWith(prefix)).map(([path, entry]) => info(path, entry)).sort((a, b) => b.savedAt - a.savedAt),
    remove: async (path) => { files.delete(cleanPath(path)); },
  };
}

/** Files in the origin private file system. Folders in a path become real directories. */
export function opfsFileStore(): FileStore {
  const root = () => navigator.storage.getDirectory();
  const directoryOf = async (path: string, create: boolean) => {
    const parts = path.split('/');
    let directory = await root();
    for (const part of parts.slice(0, -1)) directory = await directory.getDirectoryHandle(part, { create });
    return { directory, name: parts[parts.length - 1] };
  };
  const walk = async (directory: FileSystemDirectoryHandle, base: string, out: StoredFile[]) => {
    // `values()` is the async iterator over a directory's entries.
    for await (const handle of (directory as unknown as { values(): AsyncIterable<FileSystemHandle> }).values()) {
      if (handle.kind === 'directory') await walk(handle as FileSystemDirectoryHandle, `${base}${handle.name}/`, out);
      else { const file = await (handle as FileSystemFileHandle).getFile(); out.push({ path: `${base}${handle.name}`, size: file.size, type: file.type, savedAt: file.lastModified }); }
    }
  };
  return {
    async put(path, data) {
      const key = cleanPath(path);
      const { directory, name } = await directoryOf(key, true);
      const handle = await directory.getFileHandle(name, { create: true });
      const writable = await handle.createWritable();
      await writable.write(data);
      await writable.close();
      const file = await handle.getFile();
      return { path: key, size: file.size, type: data.type || file.type, savedAt: file.lastModified };
    },
    async get(path) {
      try { const { directory, name } = await directoryOf(cleanPath(path), false); return await (await directory.getFileHandle(name)).getFile(); } catch { return undefined; }
    },
    async list(prefix = '') {
      const out: StoredFile[] = [];
      await walk(await root(), '', out);
      return out.filter((file) => file.path.startsWith(prefix)).sort((a, b) => b.savedAt - a.savedAt);
    },
    async remove(path) {
      try { const { directory, name } = await directoryOf(cleanPath(path), false); await directory.removeEntry(name); } catch { /* already gone */ }
    },
  };
}

/** The best store this browser has: the private file system if it is there, otherwise memory (the session only). */
export function browserFileStore(): FileStore {
  return typeof navigator !== 'undefined' && navigator.storage && 'getDirectory' in navigator.storage ? opfsFileStore() : memoryFileStore();
}

/** Asks the browser not to clear what we keep when space runs low, and reports how much is used. Both can be refused; neither is an error. */
export async function storageStatus(): Promise<{ persistent: boolean; used: number; quota: number }> {
  try {
    const persistent = (await navigator.storage.persisted?.()) || (await navigator.storage.persist?.()) || false;
    const estimate = await navigator.storage.estimate?.();
    return { persistent, used: estimate?.usage ?? 0, quota: estimate?.quota ?? 0 };
  } catch { return { persistent: false, used: 0, quota: 0 }; }
}
