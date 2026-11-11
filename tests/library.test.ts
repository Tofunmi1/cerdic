import { describe, expect, test } from 'bun:test';
import { byRecent, createProject, duplicateProject, exportLibrary, memoryProjectStore, parseLibrary, renameProject, uniqueName } from '../src/studio/library/projects';
import { cleanPath, freshPath, humanSize, memoryFileStore } from '../src/studio/library/files';

describe('project library', () => {
  test('names never clash, whatever the case', () => {
    expect(uniqueName([], 'House')).toBe('House');
    expect(uniqueName(['House'], 'house')).toBe('house 2');
    expect(uniqueName(['House', 'House 2'], 'House')).toBe('House 3');
    expect(uniqueName([], '   ')).toBe('Untitled');
  });

  test('create, duplicate and rename keep names unique and stamp the time', () => {
    const a = createProject([], 'sky = scene.sky@1()', 'House', 'blank', 1000);
    const b = createProject([a], 'x', 'House', 'file', 2000);
    expect(b.name).toBe('House 2');
    const copy = duplicateProject([a, b], a, 3000);
    expect(copy.name).toBe('House copy');
    expect(copy.id).not.toBe(a.id);
    expect(a.worldId).toBeTruthy();
    expect(copy.worldId).not.toBe(a.worldId);
    expect(copy.text).toBe(a.text);
    expect(renameProject([a, b], b, 'House', 4000).name).toBe('House 2'); // "House" is taken by the other project; the project's own old name does not count
    expect(renameProject([a, b], b, 'Barn', 4000).name).toBe('Barn');
    expect(renameProject([a, b], b, 'Barn', 4000).worldId).toBe(b.worldId);
    expect(renameProject([a, b], b, '   ', 4000)).toBe(b); // an empty name is refused
  });

  test('the newest change comes first', () => {
    const a = { ...createProject([], 'a', 'A'), updatedAt: 1 };
    const b = { ...createProject([a], 'b', 'B'), updatedAt: 5 };
    expect(byRecent([a, b]).map((p) => p.name)).toEqual(['B', 'A']);
  });

  test('the store keeps what it is given and forgets what it is told to', async () => {
    const store = memoryProjectStore();
    const a = createProject([], 'text', 'A');
    await store.put(a);
    expect((await store.get(a.id))?.text).toBe('text');
    await store.put({ ...a, text: 'changed' });
    expect(await store.list()).toHaveLength(1);
    expect((await store.get(a.id))?.text).toBe('changed');
    await store.remove(a.id);
    expect(await store.list()).toHaveLength(0);
  });

  test('a library file round-trips and refuses anything else', () => {
    const projects = [createProject([], 'a', 'A'), createProject([], 'b', 'B')];
    expect(parseLibrary(exportLibrary(projects)).map((p) => p.name)).toEqual(['A', 'B']);
    expect(() => parseLibrary('{"hello":1}')).toThrow('not a Cerdic library');
    expect(parseLibrary(JSON.stringify({ format: 'cerdic-library', projects: [{ id: 1 }, projects[0]] }))).toHaveLength(1); // a broken entry is dropped
  });
});

describe('file store', () => {
  test('paths are made safe', () => {
    expect(cleanPath('exports/../../etc/passwd')).toBe('exports/etc/passwd');
    expect(cleanPath('/a//b/./c.glb')).toBe('a/b/c.glb');
    expect(cleanPath('my file?.txt')).toBe('my file-.txt');
    expect(() => cleanPath('..')).toThrow();
  });

  test('a taken name gets a number before the extension', () => {
    expect(freshPath([], 'house.glb')).toBe('house.glb');
    expect(freshPath(['house.glb'], 'house.glb')).toBe('house 2.glb');
    expect(freshPath(['exports/house.glb', 'exports/house 2.glb'], 'exports/house.glb')).toBe('exports/house 3.glb');
    expect(freshPath(['notes'], 'notes')).toBe('notes 2');
  });

  test('put, get, list by folder and remove', async () => {
    let clock = 0;
    const store = memoryFileStore(() => ++clock);
    await store.put('exports/a.glb', new Blob(['aaaa'], { type: 'model/gltf-binary' }));
    await store.put('snapshots/b.jpg', new Blob(['bb'], { type: 'image/jpeg' }));
    expect((await store.list()).map((f) => f.path)).toEqual(['snapshots/b.jpg', 'exports/a.glb']); // newest first
    expect((await store.list('exports/')).map((f) => f.path)).toEqual(['exports/a.glb']);
    const file = await store.get('exports/a.glb');
    expect(file?.size).toBe(4);
    expect(await file?.text()).toBe('aaaa');
    await store.remove('exports/a.glb');
    expect(await store.get('exports/a.glb')).toBeUndefined();
  });

  test('sizes read like a person would say them', () => {
    expect(humanSize(512)).toBe('512 B');
    expect(humanSize(2048)).toBe('2.0 KB');
    expect(humanSize(5 * 1024 * 1024)).toBe('5.0 MB');
  });
});
