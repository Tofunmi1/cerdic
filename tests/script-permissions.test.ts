import { beforeEach, expect, test } from 'bun:test';
import { createScriptHost } from '../src/engine/scripts';

// A stand-in for the browser's Worker: it records what the host sends it, and the test plays the script by sending the host the messages a script's code would.
class FakeWorker {
  static all: FakeWorker[] = [];
  onmessage: ((event: MessageEvent) => void) | null = null;
  onerror: ((event: { message: string }) => void) | null = null;
  sent: Array<Record<string, unknown>> = [];
  constructor(_url: string) { FakeWorker.all.push(this); }
  postMessage(message: Record<string, unknown>) { this.sent.push(message); }
  addEventListener() {}
  removeEventListener() {}
  terminate() {}
  say(data: Record<string, unknown>) { this.onmessage?.({ data } as MessageEvent); }
}
const globals = globalThis as unknown as Record<string, unknown>;
globals.Worker = FakeWorker;
globals.requestAnimationFrame = () => 1;

function fixture() {
  const calls: string[] = [];
  const ui: string[] = [];
  let teleports = 0;
  const runtime = { world_call: (method: string) => { calls.push(method); return calls.length; }, world_take: () => undefined };
  const host = createScriptHost(runtime, { ui: (_name, message) => { ui.push(message.op); return null; }, teleport: () => { teleports++; }, pose: () => ({ position: [0, 0, 0] }) });
  return { host, calls, ui, teleports: () => teleports };
}
beforeEach(() => { FakeWorker.all = []; });
const lastWorker = () => FakeWorker.all[FakeWorker.all.length - 1];
const lastReply = (w: FakeWorker) => [...w.sent].reverse().find((m) => m.type === 'reply') as { ok: boolean; error?: string } | undefined;

test('a script that is not from a pack keeps the page\'s own rules: any world method, UI and the view', () => {
  const { host, calls, ui, teleports } = fixture();
  host.start('plain', 'x', 'tool');
  const w = lastWorker();
  w.say({ type: 'call', id: 1, method: 'world.insert_components', params: {} });
  w.say({ type: 'ui', op: 'set', id: 'a', spec: {} });
  w.say({ type: 'page', op: 'teleport', position: [0, 0, 0], target: [0, 0, 1] });
  expect(calls).toEqual(['world.insert_components']);
  expect(ui).toEqual(['set']);
  expect(teleports()).toBe(1);
});

test('a pack script may call exactly the world methods its manifest grants, and is told so when it calls another', () => {
  const { host, calls } = fixture();
  host.start('boss', 'x', 'tool', { world: ['game.spawn', 'world.get_*'.replace('get_*', 'parts')], ui: false, view: false });
  const w = lastWorker();
  w.say({ type: 'call', id: 1, method: 'game.spawn', params: {} });
  expect(calls).toEqual(['game.spawn']);
  w.say({ type: 'call', id: 2, method: 'world.insert_components', params: {} });
  w.say({ type: 'call', id: 3, method: 'game.place', params: {} });
  expect(calls).toEqual(['game.spawn']); // the other two never reached the world
  const reply = lastReply(w);
  expect(reply?.ok).toBe(false);
  expect(reply?.error).toContain('permission denied');
  expect(host.logs('boss')).toMatchObject({ ok: true });
  const errors = (host.logs('boss') as { errors: string[] }).errors;
  expect(errors.filter((e) => e.startsWith('permission denied')).length).toBe(2);
});

test('a family permission grants its methods and not a lookalike', () => {
  const { host, calls } = fixture();
  host.start('fam', 'x', 'tool', { world: ['game.*'], ui: false, view: false });
  const w = lastWorker();
  w.say({ type: 'call', id: 1, method: 'game.spawn', params: {} });
  w.say({ type: 'call', id: 2, method: 'game.place', params: {} });
  w.say({ type: 'call', id: 3, method: 'gamer.spawn', params: {} });
  expect(calls).toEqual(['game.spawn', 'game.place']);
});

test('UI and the view are denied unless granted, and a pose request is answered with the refusal', () => {
  const { host, ui, teleports } = fixture();
  host.start('quiet', 'x', 'tool', { world: [], ui: false, view: false });
  const w = lastWorker();
  w.say({ type: 'ui', op: 'set', id: 'a', spec: {} });
  w.say({ type: 'page', op: 'teleport', position: [0, 0, 0], target: [0, 0, 1] });
  w.say({ type: 'page', id: 7, op: 'pose' });
  expect(ui).toEqual([]);
  expect(teleports()).toBe(0);
  const reply = lastReply(w);
  expect(reply?.ok).toBe(false);
  expect(reply?.error).toContain('view is not granted');
  // Granted, the same requests work.
  host.start('loud', 'x', 'tool', { world: [], ui: true, view: true });
  const w2 = lastWorker();
  w2.say({ type: 'ui', op: 'set', id: 'a', spec: {} });
  w2.say({ type: 'page', op: 'teleport', position: [0, 0, 0], target: [0, 0, 1] });
  w2.say({ type: 'page', id: 8, op: 'pose' });
  expect(ui).toEqual(['set']);
  expect(teleports()).toBe(1);
  expect(lastReply(w2)?.ok).toBe(true);
});

test('installing a pack starts its scripts with their manifest permissions, and an invalid pack starts nothing', async () => {
  const { installPack } = await import('../src/engine/pack-loader');
  const { sha256 } = await import('../src/engine/packs');
  const source = "on('frame', async () => { await world.call('game.spawn', {}); });\n";
  const macro = 'def make(src)\n  a_{src} = solid.box@1(size=[1, 1, 1])\n  output {src} = a_{src}\nend\n';
  const manifest = {
    name: 'pk', version: '1.0.0', description: 'A macro and a script that may only spawn things.',
    requires: { operators: ['solid.box@1'] }, provides: { macros: ['make'], groups: [], scripts: ['boss'] },
    files: { 'pk.txt': { kind: 'macros' as const, sha256: await sha256(macro) }, 'boss.js': { kind: 'script' as const, sha256: await sha256(source) } },
    permissions: { scripts: { boss: { world: ['game.spawn'], ui: false, view: false } } },
  };
  const engine = { operators: new Set(['solid.box@1']) };
  const { host, calls } = fixture();
  const bad = await installPack(host, manifest, { 'pk.txt': macro, 'boss.js': `${source}// tampered\n` }, engine);
  expect(bad.ok).toBe(false);
  expect(host.list()).toEqual([]); // nothing started
  const good = await installPack(host, manifest, { 'pk.txt': macro, 'boss.js': source }, engine);
  expect(good.ok).toBe(true);
  if (good.ok) { expect(good.pack.scripts).toEqual(['pk_boss']); expect(good.pack.text).toContain('def make('); }
  expect(host.list().map((s) => s.name)).toEqual(['pk_boss']);
  const w = lastWorker();
  w.say({ type: 'call', id: 1, method: 'game.spawn', params: {} });
  w.say({ type: 'call', id: 2, method: 'world.insert_components', params: {} });
  expect(calls).toEqual(['game.spawn']);
});

test('one script\'s permissions do not leak to another running beside it', () => {
  const { host, calls } = fixture();
  host.start('a', 'x', 'tool', { world: ['game.spawn'], ui: false, view: false });
  const a = lastWorker();
  host.start('b', 'x', 'tool', { world: [], ui: false, view: false });
  const b = lastWorker();
  b.say({ type: 'call', id: 1, method: 'game.spawn', params: {} });
  expect(calls).toEqual([]);
  a.say({ type: 'call', id: 1, method: 'game.spawn', params: {} });
  expect(calls).toEqual(['game.spawn']);
});
