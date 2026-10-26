const textOf = (result) => result.content.filter((block) => block.type === 'text').map((block) => block.text).join('\n');

/** Run a bounded list of tool calls through the server's normal dispatcher. */
export async function callBatch(args, callTool) {
  const steps = Array.isArray(args?.steps) ? args.steps : [];
  if (steps.length > 32) return { content: [{ type: 'text', text: 'batch accepts at most 32 steps' }], isError: true };
  const maxChars = Number.isInteger(args?.max_chars) && args.max_chars >= 0 ? args.max_chars : 3000;
  const stopOnError = args?.stop_on_error !== false;
  const entries = [];
  const images = [];
  let stoppedAt;

  for (let i = 0; i < steps.length; i++) {
    const step = steps[i] ?? {};
    const tool = String(step.tool ?? '');
    let result;
    if (tool === 'batch' || tool === 'run') {
      result = { content: [{ type: 'text', text: `${tool} may not be called inside batch` }], isError: true };
    } else {
      result = await callTool(tool, step.args ?? {});
    }
    const text = textOf(result);
    const entry = { tool, ok: result.isError !== true };
    if (text) {
      entry.text = text.slice(0, maxChars);
      if (text.length > maxChars) entry.truncated = true;
    }
    if (!entry.ok) entry.error = text;
    if (i === steps.length - 1) images.push(...result.content.filter((block) => block.type === 'image'));
    entries.push(entry);
    if (!entry.ok && stopOnError) { stoppedAt = i; break; }
  }

  const answer = { steps: entries, ...(stoppedAt === undefined ? {} : { stopped_at: stoppedAt }) };
  return { content: [{ type: 'text', text: JSON.stringify(answer) }, ...images], isError: false };
}
