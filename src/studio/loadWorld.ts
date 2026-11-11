// Putting a program in the text language into the studio as the document, from wherever it came: an example, a file the person picked or dropped, or a URL.
import { host } from '../engine/host';
import { documentName, documentSeed } from './documentIdentity';
export { documentName } from './documentIdentity';

const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

/**
 * Loads `text` as a fresh document once the evaluator is idle. The engine keeps the last document in the browser, so the world goes into a new one:
 * otherwise it would be added to whatever was already in the scene.
 */
export async function loadWorldText(text: string, name = 'spatial-lab'): Promise<void> {
  while (!(host.ready && host.evaluation?.state.worker === 'idle')) await sleep(150);
  host.runtime?.doc_reset(documentName(name), documentSeed(name));
  host.evaluation?.request();
  if (!text.trim()) return;
  const result = await host.doc?.sendText(text, 'world');
  if (result && 'diagnostics' in result) throw new Error(result.diagnostics.map(d => d.message).join('\n') || 'Scene text could not be parsed.');
  const outcome = result && 'pending' in result ? result.pending : result;
  if (outcome && 'rejected' in outcome) throw new Error(outcome.rejected.diagnostics.map(d => d.message).join('\n'));
  if (outcome && 'conflict' in outcome) throw new Error('The scene changed while it was loading. Try again.');
}

/** Reads a file the person chose or dropped, and loads it. Returns false for something that is not a text program. */
export async function loadWorldFile(file: File): Promise<boolean> {
  if (!/\.(txt|world|ve)$/i.test(file.name) && !file.type.startsWith('text/')) return false;
  await loadWorldText(await file.text(), file.name);
  return true;
}

/** Saves the document as a `.txt` program the person can keep, share or open again. */
export function downloadDocumentText(name: string): boolean {
  const result = host.doc?.text();
  if (!result?.text) return false;
  const url = URL.createObjectURL(new Blob([result.text], { type: 'text/plain' }));
  const link = document.createElement('a');
  link.href = url;
  link.download = `${documentName(name)}.txt`;
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
  return true;
}
