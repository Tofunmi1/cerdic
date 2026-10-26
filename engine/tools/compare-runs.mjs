// bun tools/compare-runs.mjs <name>...
import { readFileSync } from 'node:fs';
import { aggregate, formatTable } from './mcp/run-metrics.mjs';

const names = process.argv.slice(2);
if (!names.length) {
  console.error('Usage: bun tools/compare-runs.mjs <name>...');
  process.exit(1);
}
const runs = names.map((name) => {
  const saved = JSON.parse(readFileSync(new URL(`../web/dist-bevy-gpu/worlds/${encodeURIComponent(name)}.json`, import.meta.url), 'utf8'));
  if (!saved.metrics) throw new Error(`${name} has no saved metrics`);
  return { ...saved.metrics, name };
});
console.log(formatTable(runs));
console.log(JSON.stringify(aggregate(runs), null, 2));
