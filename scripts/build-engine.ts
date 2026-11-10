import { access, cp, mkdir } from 'node:fs/promises';
import { homedir } from 'node:os';
const root = new URL('../', import.meta.url).pathname;
if (!process.argv.includes('--copy-only')) {
  const args = ['cargo', 'xtask', 'web'];
  if (process.argv.includes('--dev')) args.push('dev');
  const sdk = process.env.WASI_SDK ?? `${homedir()}/.local/share/toolchains/wasi-sdk-34.0-arm64-macos`;
  const wasiClang = `${sdk}/bin/clang`;
  const env = { ...process.env };
  if (!env.CC_wasm32_unknown_unknown) {
    try {
      await access(wasiClang);
      env.CC_wasm32_unknown_unknown = wasiClang;
    } catch {
      // Keep Cargo's configured compiler when wasi-sdk is installed elsewhere or not installed.
    }
  }
  const child = Bun.spawn(args, { cwd: `${root}engine`, env, stdout: 'inherit', stderr: 'inherit' });
  if (await child.exited !== 0) process.exit(1);
}
await mkdir(`${root}public/engine`, { recursive: true });
await cp(`${root}engine/web/editor-loader.js`, `${root}public/engine/editor-loader.js`);
for (const file of ['ve_runtime.js', 've_runtime_bg.wasm', 've_evaluator.js', 've_evaluator_bg.wasm', 'eval-worker.js', 'wasi_snapshot_preview1.js', 'snippets']) {
  await cp(`${root}engine/web/dist/${file}`, `${root}public/engine/${file}`, { recursive: true });
}
