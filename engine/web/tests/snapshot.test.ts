import { afterEach, expect, test } from 'bun:test';
import { snapshotView, type ViewHooks } from '../../../src/engine/snapshot';
import { validateToolArguments } from '../tool-arguments.js';
import { snapshotTour } from '../../../src/engine/survey';

const camera = { position: [0, 5, 5], target: [0, 0, 0] };

test('motion snapshots restore the previous clock and camera on success and capture failure', async () => {
  for (const playing of [true, false]) {
    for (const throwAt of [undefined, 1]) {
      const { hooks, moves } = viewport({ throwAt });
      const clock = { seconds: 7, playing };
      const captured: number[] = [];
      hooks.clock = () => ({ ...clock });
      hooks.play = (on, seconds) => { clock.playing = on; if (seconds !== undefined) clock.seconds = seconds; };
      hooks.seek = seconds => { clock.seconds = seconds; captured.push(seconds); };
      hooks.setParam = () => {};
      const result = snapshotView({ times: [0, 1], image: false }, hooks);
      if (throwAt) await expect(result).rejects.toThrow('capture failed');
      else { expect((await result).ok).toBe(true); expect(captured).toEqual([0, 1]); }
      expect(clock).toEqual({ seconds: 7, playing });
      expect(moves.at(-1)?.position).toEqual([7, 8, 9]);
    }
  }
});

test('explicit snapshot cameras validate finite vectors, counts and every mode conflict', () => {
  expect(validateToolArguments('snapshot', { camera })).toBeNull();
  expect(validateToolArguments('snapshot', { cameras: Array(6).fill(camera) })).toBeNull();
  for (const cameras of [[], Array(7).fill(camera), null, {}]) {
    expect(validateToolArguments('snapshot', { cameras })).toContain('1 to 6');
  }
  for (const mode of ['camera', 'cameras']) {
    for (const field of ['position', 'target']) {
      for (const vector of [[], [1, 2], [1, 2, 3, 4], [1, NaN, 3], [1, Infinity, 3], [1, '2', 3], null]) {
        const bad = { ...camera, [field]: vector };
        expect(validateToolArguments('snapshot', { [mode]: mode === 'camera' ? bad : [bad] })).toContain(`${field} must be [x, y, z]`);
      }
    }
    for (const other of ['views', 'times', 'states', mode === 'camera' ? 'cameras' : 'camera']) {
      const error = validateToolArguments('snapshot', { [mode]: mode === 'camera' ? camera : [camera], [other]: [] });
      expect(error).toContain(mode);
      expect(error).toContain(other);
    }
    expect(validateToolArguments('snapshot', { [mode]: null })).not.toBeNull();
  }
  expect(validateToolArguments('snapshot', { cameras: [{ ...camera, label: 1 }] })).toContain('label');
});

const savedGlobals = ['document', 'requestAnimationFrame'].map(name => [name, Object.getOwnPropertyDescriptor(globalThis, name)] as const);
afterEach(() => {
  for (const [name, descriptor] of savedGlobals) {
    if (descriptor) Object.defineProperty(globalThis, name, descriptor);
    else Reflect.deleteProperty(globalThis, name);
  }
});

// A canvas/clock fixture exercises the real capture and composition path without a browser or renderer.
function viewport(options: { failAt?: number; throwAt?: number; rejectCamera?: boolean } = {}) {
  const moves: Array<{ position: number[]; target: number[] }> = [];
  const captures: number[][] = [];
  const labels: string[] = [];
  let position = [7, 8, 9];
  let count = 0;
  const source = { width: 640, height: 480, clientWidth: 640, clientHeight: 480 };
  const hooks: ViewHooks = {
    lookAt: (p, t) => { position = [...p]; moves.push({ position, target: [...t] }); return !options.rejectCamera; },
    pose: () => ({ position: [7, 8, 9], rotation: [0, 0, 0, 1] }),
    bounds: () => ({ min: [-1, -1, -1], max: [1, 1, 1] }),
    partBounds: () => null,
  };
  const document = {
    querySelectorAll: () => [source],
    createElement: () => {
      const index = ++count;
      const canvas = {
        width: 0, height: 0,
        getContext: () => index === options.failAt ? null : {
          drawImage: (image: unknown) => { if (image === source) captures.push([...position]); },
          getImageData: () => {
            if (index === options.throwAt) throw new Error('capture failed');
            return { data: new Uint8ClampedArray(canvas.width * canvas.height * 4).fill(255) };
          },
          fillRect: () => {},
          fillText: (label: string) => { labels.push(label); },
        },
        toDataURL: () => 'data:image/jpeg;base64,cGljdHVyZQ==',
      };
      return canvas;
    },
  };
  Object.defineProperty(globalThis, 'document', { configurable: true, value: document });
  Object.defineProperty(globalThis, 'requestAnimationFrame', { configurable: true, value: (callback: FrameRequestCallback) => { queueMicrotask(() => callback(0)); return 1; } });
  return { hooks, moves, captures, labels };
}

test('camera sheets capture each explicit position, label the shared layout and restore the previous pose', async () => {
  const f = viewport();
  f.hooks.bounds = () => null; // Explicit cameras work without any bounds to frame.
  const cameras = [camera, { position: [5, 2, 0], target: [1, 0, 0], label: 'door' }, { ...camera, label: 'roof' }];
  const result = await snapshotView({ cameras }, f.hooks);
  expect(result).toMatchObject({ ok: true, width: 512, height: 384, jpegBase64: 'cGljdHVyZQ==' });
  expect(f.labels).toEqual(['camera 1', 'door', 'roof']);
  expect(f.captures).toEqual(cameras.map(c => c.position));
  expect(f.moves.slice(0, -1)).toEqual(cameras.map(({ position, target }) => ({ position, target })));
  expect(f.moves.at(-1)).toEqual({ position: [7, 8, 9], target: [7, 8, 8] });
});

test('a single camera returns one baseline-comparable picture and restores the pose', async () => {
  const f = viewport();
  const params = { camera, name: 'single-camera-unit-test', width: 320 };
  expect(await snapshotView(params, f.hooks)).toMatchObject({ ok: true, width: 320, height: 240, camera, jpegBase64: 'cGljdHVyZQ==' });
  const next = await snapshotView({ ...params, image: false }, f.hooks);
  expect(next).toMatchObject({ ok: true, compared: { diff: { changedFraction: 0 } } });
  expect(next).not.toHaveProperty('jpegBase64');
  expect(f.labels).toEqual([]);
  expect(f.captures).toEqual([camera.position, camera.position]);
  expect(f.moves.at(-1)).toEqual({ position: [7, 8, 9], target: [7, 8, 8] });
});

test('named views retain their layout, labels and restoration', async () => {
  const f = viewport();
  expect(await snapshotView({ views: ['front', 'side', 'top'], image: false }, f.hooks)).toMatchObject({ ok: true, views: ['front', 'side', 'top'], width: 512, height: 384 });
  expect(f.labels).toEqual(['front', 'side', 'top']);
  expect(f.moves.at(-1)).toEqual({ position: [7, 8, 9], target: [7, 8, 8] });
});

test('single and multiple cameras restore on capture errors and rejected moves', async () => {
  for (const mode of [{ camera }, { cameras: [camera, camera] }]) {
    for (const options of [{ failAt: 1 }, { throwAt: 1 }, { rejectCamera: true }]) {
      const f = viewport(options);
      if (options.throwAt) await expect(snapshotView(mode, f.hooks)).rejects.toThrow('capture failed');
      else expect(await snapshotView(mode, f.hooks)).toMatchObject({ ok: false });
      expect(f.moves.at(-1)).toEqual({ position: [7, 8, 9], target: [7, 8, 8] });
    }
  }
  const f = viewport({ failAt: 2 });
  expect(await snapshotView({ cameras: [camera, camera] }, f.hooks)).toMatchObject({ ok: false });
  expect(f.captures).toHaveLength(1);
  expect(f.moves.at(-1)).toEqual({ position: [7, 8, 9], target: [7, 8, 8] });
});

test('invalid modes fail on the page before moving or capturing', async () => {
  const f = viewport();
  expect(await snapshotView({ cameras: [{ position: [1, 2], target: [0, 0, 0] }] }, f.hooks)).toMatchObject({ ok: false, reason: expect.stringContaining('position must be [x, y, z]') });
  expect(f.moves).toEqual([]);
  expect(f.captures).toEqual([]);
  expect(await snapshotView({ camera })).toMatchObject({ ok: false, reason: 'this page cannot move the camera' });
});

test('survey tours capture twelve labelled cameras in two sheets and restore each time', async () => {
  const f = viewport();
  const cameras = Array.from({ length: 12 }, (_, i) => ({ position: [i, 1.65, 0], target: [i, 1.65, 1], label: `stand_${i + 1}` }));
  const runtime = { survey_plan: (json: string) => {
    expect(JSON.parse(json)).toEqual({ max_views: 12 });
    return JSON.stringify({ cameras, coverage: 0.95 });
  } };
  const result = await snapshotTour(runtime, { tour: { max_views: 12 } }, f.hooks);
  expect(result.ok).toBe(true);
  expect(result.sheets).toHaveLength(2);
  for (const sheet of result.sheets ?? []) expect(sheet).toMatchObject({ ok: true, width: 512, height: 576, jpegBase64: 'cGljdHVyZQ==' });
  expect(f.labels).toEqual(cameras.map(c => c.label));
  expect(f.captures).toEqual(cameras.map(c => c.position));
  expect(f.moves[6]).toEqual({ position: [7, 8, 9], target: [7, 8, 8] });
  expect(f.moves.at(-1)).toEqual(f.moves[6]);
});

test('tour capture failures restore the camera and invalid requests never reach the export', async () => {
  const cameras = Array.from({ length: 8 }, (_, i) => ({ ...camera, label: `aerial_${i + 1}` }));
  const runtime = { survey_plan: () => JSON.stringify({ cameras, coverage: 1 }) };
  const f = viewport({ failAt: 9 });
  const result = await snapshotTour(runtime, { tour: true, image: false }, f.hooks);
  expect(result).toMatchObject({ ok: false, reason: 'the page cannot read back the viewport' });
  expect(result.sheets).toHaveLength(2);
  expect(result.sheets?.[0]).not.toHaveProperty('jpegBase64');
  expect(f.moves.at(-1)).toEqual({ position: [7, 8, 9], target: [7, 8, 8] });
  const bad = { survey_plan: () => { throw new Error('must not call export'); } };
  expect(await snapshotTour(bad, { tour: { max_views: 13 } }, f.hooks)).toMatchObject({ ok: false });
  expect(await snapshotTour(runtime, { tour: true, cameras }, f.hooks)).toMatchObject({ ok: false });
});

test('an empty survey returns its counts without moving or capturing', async () => {
  const f = viewport();
  const runtime = { survey_plan: (json: string) => {
    expect(JSON.parse(json)).toEqual({});
    return JSON.stringify({ cameras: [], coverage: 0, triangles: 0, surface_samples: 0, standable_points: 0 });
  } };
  expect(await snapshotTour(runtime, { tour: true }, f.hooks)).toMatchObject({
    ok: false, reason: 'there is no geometry to survey', survey: { cameras: [], triangles: 0 },
  });
  expect(f.moves).toEqual([]);
  expect(f.captures).toEqual([]);
});
