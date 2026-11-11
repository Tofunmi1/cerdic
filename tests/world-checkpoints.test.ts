import { expect, test } from 'bun:test';
import { checkpointWorld, readCheckpoint, recoverCheckpoint } from '../src/studio/library/checkpoints';
import { createProject } from '../src/studio/library/projects';
import { documentSeed } from '../src/studio/documentIdentity';
test('world runtime identities are stable and do not reuse the legacy bootstrap seed', () => {
  expect(documentSeed('world-one')).toBe(documentSeed('world-one'));
  expect(documentSeed('world-one')).not.toBe(documentSeed('world-two'));
  expect(documentSeed('world-one')).toBeGreaterThan(1);
});
function storage() { const map = new Map<string, string>(); return { map, getItem: (key: string) => map.get(key) ?? null, setItem: (key: string, value: string) => { map.set(key, value); } }; }
test('a committed world survives reload before the asynchronous project save completes', () => {
  const store = storage(), project = createProject([], '', 'House', 'blank', 100);
  checkpointWorld(store, project, 'house = solid.box@1()\noutput house', 18, 200);
  expect(recoverCheckpoint(store, JSON.parse(JSON.stringify(project))).text).toContain('house');
  expect(recoverCheckpoint(store, project).id).toBe(project.id);
});
test('checkpoints cannot leak across projects or worlds', () => {
  const store = storage(), a = createProject([], '', 'A', 'blank', 100), b = createProject([], '', 'B', 'blank', 100);
  checkpointWorld(store, a, 'world A', 2, 200);
  expect(recoverCheckpoint(store, b)).toBe(b);
  expect(readCheckpoint(store, { ...a, worldId: 'another-world' })).toBeUndefined();
});
test('an intentional committed empty world replaces an older nonempty save', () => {
  const store = storage(), project = createProject([], 'old house', 'House', 'file', 100);
  checkpointWorld(store, project, '', 19, 200);
  expect(recoverCheckpoint(store, project).text).toBe('');
});
test('older or corrupt checkpoint data cannot replace a newer saved project', () => {
  const store = storage(), project = createProject([], 'new house', 'House', 'file', 300);
  checkpointWorld(store, project, 'old house', 1, 200);
  expect(recoverCheckpoint(store, project)).toBe(project);
  for (const key of store.map.keys()) store.setItem(key, '{broken');
  expect(recoverCheckpoint(store, project)).toBe(project);
});
test('storage failure is surfaced to the caller, never reported as a checkpoint success', () => {
  const project = createProject([], '', 'House');
  expect(() => checkpointWorld({ getItem: () => null, setItem: () => { throw new Error('quota'); } }, project, 'house', 1)).toThrow('quota');
});
