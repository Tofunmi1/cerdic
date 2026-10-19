/** Parses Material Maker's lenient JSON (a few source `.mmg` definitions contain trailing commas). */
export function parseLenientJson<T>(text: string): T {
  let out = '';
  let inString = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (inString) {
      out += c;
      if (c === '\\') out += text[++i] ?? '';
      else if (c === '"') inString = false;
    } else if (c === '"') {
      inString = true;
      out += c;
    } else if (c === ',') {
      let k = i + 1;
      while (k < text.length && /\s/.test(text[k])) k++;
      if (text[k] !== '}' && text[k] !== ']') out += c;
    } else out += c;
  }
  return JSON.parse(out) as T;
}
