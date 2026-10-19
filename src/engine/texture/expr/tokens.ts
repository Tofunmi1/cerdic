// Splits shader expression source into tokens. A newline outside a comment becomes a `;` separator.
import { Failure } from './types';

export type Tok = { value: string; pos: number };

/** Words that exist in GLSL or other languages but not here; using one is a syntax error, not an unknown name. */
export const RESERVED = new Set([
  'if', 'else', 'while', 'do', 'switch', 'case', 'return', 'break', 'continue', 'discard',
  'float', 'int', 'bool', 'uniform', 'varying', 'const', 'struct', 'void', 'function', 'var',
]);

const NUMBER = /^(?:\d+(?:\.(?![.A-Za-z])\d*)?|\.\d+)(?:[eE][+-]?\d+)?/;
const PUNCTUATION = '+-*/%(){};,.=';

export function tokenize(source: string): Tok[] {
  const out: Tok[] = [];
  let i = 0;
  while (i < source.length) {
    const c = source[i];
    if (c === '\n') {
      if (out.at(-1)?.value !== ';') out.push({ value: ';', pos: i });
      i++;
    } else if (/\s/.test(c)) {
      i++;
    } else if (c === '/' && source[i + 1] === '/') {
      while (i < source.length && source[i] !== '\n') i++;
    } else if (/[A-Za-z_]/.test(c)) {
      const start = i++;
      while (i < source.length && /[A-Za-z0-9_]/.test(source[i])) i++;
      out.push({ value: source.slice(start, i), pos: start });
    } else if (source.startsWith('..', i)) {
      out.push({ value: '..', pos: i });
      i += 2;
    } else if (/[0-9]/.test(c) || (c === '.' && /[0-9]/.test(source[i + 1] ?? ''))) {
      const match = NUMBER.exec(source.slice(i));
      if (!match) throw new Failure('expr.syntax', 'Invalid number', 'Write a decimal number such as 0.5.', i);
      out.push({ value: match[0], pos: i });
      i += match[0].length;
    } else if (PUNCTUATION.includes(c)) {
      out.push({ value: c, pos: i++ });
    } else if ('<>!&|?:'.includes(c)) {
      throw new Failure('expr.syntax', `Unsupported operator '${c}'`, 'Use arithmetic (+ - * / %) and functions such as step() or mix() instead of comparisons.', i);
    } else {
      throw new Failure('expr.syntax', `Unexpected '${c}'`, 'Remove the character; only numbers, names and + - * / % ( ) , . = ; are allowed.', i);
    }
  }
  return out;
}
