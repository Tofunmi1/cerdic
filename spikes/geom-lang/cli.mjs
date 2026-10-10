import { compile, spec } from './lang.mjs';
import fs from 'node:fs';
const [cmd, file] = process.argv.slice(2);
if (cmd === 'spec') console.log(spec());
else { const r = compile(fs.readFileSync(file, 'utf8')); console.log(JSON.stringify({ ok: r.ok, nodes: r.count, errors: r.errors, warnings: r.warnings })); }
