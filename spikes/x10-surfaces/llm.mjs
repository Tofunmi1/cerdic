// Calls the local Claude Code CLI as a plain completion endpoint: no tools, no project settings,
// no session persistence. Returns text (or parsed structured output) plus cost, tokens and time.
import { spawn } from 'node:child_process';
import os from 'node:os';

const CWD = os.tmpdir();

export function ask({ model, system, prompt, schema, timeoutMs = 240000 }) {
  const args = ['-p', '--tools', '', '--setting-sources', '', '--disable-slash-commands', '--no-session-persistence',
    '--strict-mcp-config', '--system-prompt', system, '--model', model, '--output-format', 'json'];
  if (schema) args.push('--json-schema', JSON.stringify(schema));
  return new Promise((resolve) => {
    const t0 = Date.now();
    const child = spawn('claude', args, { cwd: CWD, stdio: ['pipe', 'pipe', 'pipe'] });
    let out = '', err = '';
    const timer = setTimeout(() => { child.kill('SIGKILL'); }, timeoutMs);
    child.stdout.on('data', (d) => { out += d; });
    child.stderr.on('data', (d) => { err += d; });
    child.on('close', () => {
      clearTimeout(timer);
      const ms = Date.now() - t0;
      try {
        const j = JSON.parse(out);
        resolve({
          ok: !j.is_error, ms, text: typeof j.result === 'string' ? j.result : '', structured: j.structured_output ?? null,
          cost: j.total_cost_usd ?? 0, inTokens: (j.usage?.input_tokens ?? 0) + (j.usage?.cache_creation_input_tokens ?? 0) + (j.usage?.cache_read_input_tokens ?? 0),
          outTokens: j.usage?.output_tokens ?? 0, raw: j,
        });
      } catch {
        resolve({ ok: false, ms, text: '', structured: null, cost: 0, inTokens: 0, outTokens: 0, error: (err || out).slice(0, 400) });
      }
    });
    child.stdin.end(prompt);
  });
}
