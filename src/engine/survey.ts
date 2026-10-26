import { validateToolArguments } from '../../engine/web/tool-arguments.js';
import { snapshotView, type ViewHooks } from './snapshot';

type Camera = { position: number[]; target: number[]; label: string };
type Plan = {
  mode: 'walk' | 'orbit';
  cameras: Camera[]; coverage: number; predicted: number[];
  standable_points: number; surface_samples: number; triangles: number; ms: number;
  skipped_instances: number;
};
type SurveyRuntime = { survey_plan(options: string): string };

/** Survey the runtime's committed geometry and prepare the first snapshot sheet. */
export function surveyScene(runtime: SurveyRuntime, options: Record<string, unknown>) {
  const invalid = validateToolArguments('survey', options);
  if (invalid) return { error: invalid };
  const plan = JSON.parse(runtime.survey_plan(JSON.stringify(options))) as Plan | { error: string };
  if ('error' in plan) return plan;
  return { ...plan, snapshot_args: { cameras: plan.cameras.slice(0, 6) } };
}

/** Capture a survey in at most two labelled six-camera contact sheets. */
export async function snapshotTour(runtime: SurveyRuntime, params: Record<string, unknown>, hooks: ViewHooks) {
  const invalid = validateToolArguments('snapshot', params);
  if (invalid) return { ok: false, reason: invalid };
  const options = params.tour === true ? {} : params.tour as Record<string, unknown>;
  const plan = surveyScene(runtime, options);
  if ('error' in plan) return { ok: false, reason: plan.error };
  if (plan.cameras.length === 0) return { ok: false, reason: 'there is no geometry to survey', survey: plan };
  const sheets = [];
  for (let start = 0; start < Math.min(plan.cameras.length, 12); start += 6) {
    const sheet = await snapshotView({ cameras: plan.cameras.slice(start, start + 6), width: params.width, image: params.image }, hooks);
    sheets.push(sheet);
    if (!sheet.ok) return { ok: false, reason: sheet.reason, survey: plan, sheets };
  }
  return { ok: true, survey: plan, sheets };
}
