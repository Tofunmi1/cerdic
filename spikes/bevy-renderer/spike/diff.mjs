// How much each result differs from the baseline picture: share of pixels that changed by more than 8 levels, and the mean change.
import { PNG } from 'pngjs';
import { readdirSync, readFileSync } from 'node:fs';
const dir = new URL('./results/', import.meta.url).pathname;
const backend = process.argv[2] ?? 'webgl';
const load = name => PNG.sync.read(readFileSync(dir + name));
const base = load(`${backend}-baseline.png`);
for (const file of readdirSync(dir).filter(f => f.startsWith(`${backend}-`) && f.endsWith('.png') && !f.includes('baseline')).sort()) {
  const img = load(file);
  let changed = 0, sum = 0;
  for (let i = 0; i < img.data.length; i += 4) {
    const d = Math.max(Math.abs(img.data[i] - base.data[i]), Math.abs(img.data[i + 1] - base.data[i + 1]), Math.abs(img.data[i + 2] - base.data[i + 2]));
    sum += d; if (d > 8) changed++;
  }
  const px = img.data.length / 4;
  console.log(`${file.replace(`${backend}-`, '').replace('.png', '').padEnd(22)} changed ${(100 * changed / px).toFixed(1).padStart(5)}%  mean change ${(sum / px).toFixed(2)}`);
}
