// Adds the agent-link relay (relay/index.ts) to the prebuilt Vercel output: the function, and the routes that send /api/mcp/<token>, /api/relay/poll and /api/relay/result to it.
//   bun scripts/build-relay.ts            then: vercel deploy --prebuilt --prod
// Safe to run again; it replaces its own function and routes and leaves the rest of .vercel/output alone.
import { mkdir, readFile, writeFile } from 'node:fs/promises';

const root = new URL('../', import.meta.url).pathname;
const output = `${root}.vercel/output/`;
const dir = `${output}functions/api/relay.func/`;

const built = await Bun.build({ entrypoints: [`${root}relay/index.ts`], target: 'node', format: 'esm', minify: false });
if (!built.success) { for (const log of built.logs) console.error(String(log)); process.exit(1); }
await mkdir(dir, { recursive: true });
await Bun.write(`${dir}index.mjs`, built.outputs[0]);
await writeFile(`${dir}.vc-config.json`, JSON.stringify({ runtime: 'nodejs24.x', handler: 'index.mjs', launcherType: 'Nodejs', maxDuration: 300 }, null, 2));

const configFile = `${output}config.json`;
const config = JSON.parse(await readFile(configFile, 'utf8')) as { routes: Array<Record<string, unknown>> };
const ours = [
  { src: '^/api/mcp/([a-f0-9]{32})/?$', dest: '/api/relay?kind=mcp&token=$1' },
  { src: '^/api/relay/(poll|result)/?$', dest: '/api/relay?kind=$1' },
];
const routes = config.routes.filter((route) => !String(route.dest ?? '').startsWith('/api/relay'));
const at = routes.findIndex((route) => route.handle === 'filesystem');
routes.splice(at < 0 ? 0 : at, 0, ...ours);
config.routes = routes;
await writeFile(configFile, JSON.stringify(config, null, 2));
console.log(`relay function ${((await Bun.file(`${dir}index.mjs`).size) / 1024).toFixed(0)} KB, ${ours.length} routes`);
