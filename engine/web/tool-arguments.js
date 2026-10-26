// Shared boundary validation: reject malformed calls even when no editor page is connected.
const record = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const operations = 'set, rename, delete, replace';

// Kept in sync with the runtime export; both boundaries reject costly or malformed requests.
const surveyRanges = {
  eye_height: [0.1, 5], spacing: [0.25, 100], max_views: [1, 64, true],
  yaws: [1, 32, true], target_coverage: [0, 1], fov_deg: [10, 170],
  pitch_deg: [-89, 89], max_distance: [0.1, 1000], samples_per_m2: [0.01, 20],
  seed: [0, Number.MAX_SAFE_INTEGER, true],
};

function validateSurvey(args, tour = false) {
  if (!record(args)) return 'survey options must be an object';
  for (const [key, value] of Object.entries(args)) {
    if (key === 'mode') {
      if (!['auto', 'walk', 'orbit'].includes(value)) return 'mode must be one of auto, walk, orbit';
      continue;
    }
    if (!(Object.hasOwn(surveyRanges, key))) return `unknown survey option ${key}`;
    const [min, upper, integer] = surveyRanges[key];
    const max = tour && key === 'max_views' ? 12 : upper;
    if (typeof value !== 'number' || !Number.isFinite(value) || value < min || value > max || (integer && !Number.isSafeInteger(value))) return `${key} must be ${integer ? 'an integer' : 'a finite number'} from ${min} to ${max}`;
  }
  return null;
}

function validateSnapshot(args) {
  const modes = ['camera', 'cameras', 'tour'].filter(key => args[key] !== undefined);
  if (modes.length === 0) return null;
  const conflicts = [...modes, ...['views', 'times', 'states'].filter(key => args[key] !== undefined)];
  if (conflicts.length > 1) return `snapshot cannot combine ${conflicts.join(' with ')}`;
  if (args.tour !== undefined) return args.tour === true ? null : validateSurvey(args.tour, true);
  if (args.cameras !== undefined && (!Array.isArray(args.cameras) || args.cameras.length < 1 || args.cameras.length > 6)) return 'cameras must contain 1 to 6 cameras';
  const cameras = args.cameras ?? [args.camera];
  for (const [index, camera] of cameras.entries()) {
    const where = args.cameras !== undefined ? `cameras[${index}]` : 'camera';
    if (!record(camera)) return `${where} must be an object with position and target`;
    for (const key of ['position', 'target']) {
      if (!Array.isArray(camera[key]) || camera[key].length !== 3 || !camera[key].every(value => typeof value === 'number' && Number.isFinite(value))) return `${where}: ${key} must be [x, y, z] (three finite numbers)`;
    }
    if (camera.label !== undefined && typeof camera.label !== 'string') return `${where}: label must be a string`;
  }
  return null;
}

export function validateToolArguments(name, args) {
  if (!['edit', 'diff', 'snapshot', 'survey'].includes(name)) return null;
  if (!record(args)) return `${name} arguments must be an object`;
  if (name === 'survey') return validateSurvey(args);
  if (name === 'snapshot') return validateSnapshot(args);
  if (name === 'diff') {
    return args.since !== undefined && (!Number.isSafeInteger(args.since) || args.since < 0)
      ? 'since must be a non-negative safe integer revision number' : null;
  }
  if (!Array.isArray(args.ops) || args.ops.length === 0) return `ops must be a non-empty list of operations: ${operations}`;
  if (args.label !== undefined && typeof args.label !== 'string') return 'label must be a string';
  if (args.dry_run !== undefined && typeof args.dry_run !== 'boolean') return 'dry_run must be a boolean';
  for (const [index, op] of args.ops.entries()) {
    if (!record(op) || !['set', 'rename', 'delete', 'replace'].includes(op.op)) return `ops[${index}]: unknown operation ${JSON.stringify(op?.op)}; use ${operations}`;
    if (typeof op.node !== 'string' || !op.node.trim()) return `ops[${index}]: node must be a non-empty label`;
    if (op.op === 'set' && (!record(op.params) || Object.keys(op.params).length === 0)) return `ops[${index}]: set needs a non-empty params object`;
    if (op.op === 'rename' && (typeof op.to !== 'string' || !op.to.trim())) return `ops[${index}]: rename needs a non-empty to label`;
    if (op.op === 'replace' && (typeof op.text !== 'string' || !op.text.trim())) return `ops[${index}]: replace needs text containing one node statement`;
  }
  return null;
}
