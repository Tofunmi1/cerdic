import { expect, test } from 'bun:test';
import { validateToolArguments } from '../tool-arguments.js';
import { surveyScene } from '../../../src/engine/survey';

test('survey and tour accept the three modes and reject invalid modes with the choices', () => {
  for (const mode of ['auto', 'walk', 'orbit']) {
    expect(validateToolArguments('survey', { mode })).toBeNull();
    expect(validateToolArguments('snapshot', { tour: { mode } })).toBeNull();
  }
  for (const mode of ['AUTO', '', 'fly', 1, null, true, [], {}]) {
    expect(validateToolArguments('survey', { mode })).toContain('auto, walk, orbit');
    expect(validateToolArguments('snapshot', { tour: { mode } })).toContain('auto, walk, orbit');
  }
});

test('survey relays the requested mode and reports the mode actually used', () => {
  const runtime = { survey_plan: (json: string) => {
    expect(JSON.parse(json)).toEqual({ mode: 'auto' });
    return JSON.stringify({ mode: 'orbit', cameras: [], coverage: 0 });
  } };
  expect(surveyScene(runtime, { mode: 'auto' })).toMatchObject({ mode: 'orbit' });
});

test('survey and tour validate every numeric range, integer and non-finite boundary', () => {
  const ranges: Array<[string, number, number, boolean?]> = [
    ['eye_height', 0.1, 5], ['spacing', 0.25, 100], ['max_views', 1, 64, true],
    ['yaws', 1, 32, true], ['target_coverage', 0, 1], ['fov_deg', 10, 170],
    ['pitch_deg', -89, 89], ['max_distance', 0.1, 1000], ['samples_per_m2', 0.01, 20],
    ['seed', 0, Number.MAX_SAFE_INTEGER, true],
  ];
  for (const [key, min, max, integer] of ranges) {
    for (const value of [min, max]) expect(validateToolArguments('survey', { [key]: value })).toBeNull();
    for (const value of [NaN, Infinity, -Infinity, min - 1, max + 1, null, '1', true, ...(integer ? [1.5] : [])]) {
      expect(validateToolArguments('survey', { [key]: value })).toContain(`${min} to ${max}`);
    }
  }
  for (const tour of [true, {}, { max_views: 12, seed: 99 }]) expect(validateToolArguments('snapshot', { tour })).toBeNull();
  for (const tour of [false, null, [], 'true', { max_views: 13 }, { yaws: 0 }]) expect(validateToolArguments('snapshot', { tour })).not.toBeNull();
  for (const mode of ['camera', 'cameras', 'views', 'times', 'states']) {
    expect(validateToolArguments('snapshot', { tour: true, [mode]: [] })).toContain(mode);
  }
  for (const key of ['unknown', '__proto__', 'constructor']) expect(validateToolArguments('survey', JSON.parse(`{"${key}":1}`))).toContain('unknown survey option');
});

test('survey relays options to the export and returns six directly pasteable snapshot cameras', () => {
  const cameras = Array.from({ length: 8 }, (_, i) => ({ position: [i, 1.65, 0], target: [i, 1.65, 1], label: `stand_${i + 1}` }));
  const options = { spacing: 3, seed: 99 };
  const runtime = { survey_plan: (json: string) => {
    expect(JSON.parse(json)).toEqual(options);
    return JSON.stringify({ cameras, coverage: 0.96, predicted: cameras.map(() => 0.96), triangles: 100, ms: 5, skipped_instances: 2, standable_points: 8, surface_samples: 200 });
  } };
  expect(surveyScene(runtime, options)).toMatchObject({ cameras, snapshot_args: { cameras: cameras.slice(0, 6) }, coverage: 0.96, triangles: 100, skipped_instances: 2 });
  expect(surveyScene({ survey_plan: () => { throw new Error('must not call export'); } }, { spacing: -1 })).toMatchObject({ error: expect.stringContaining('0.25 to 100') });
  expect(surveyScene({ survey_plan: () => '{"error":"not ready"}' }, {})).toEqual({ error: 'not ready' });
});
