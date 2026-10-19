import { expect, test } from 'bun:test';
// @ts-expect-error: a plain JavaScript module of the MCP server, without type declarations.
import { TOPICS, coreWithMenu, splitGuide, topicText } from '../engine/tools/mcp/guide-topics.mjs';

const topics = [
  { id: 'one', start: 'First topic', summary: 'the first' },
  { id: 'two', start: 'Second topic', summary: 'the second' },
  { id: 'three', start: 'Third topic', summary: 'the third' },
];
const guide = 'Core text about syntax.\n\nMore core.\n\nFirst topic: alpha.\n\nStill alpha.\n\nSecond topic: beta.\n\nThird topic: gamma.\n';

test('the core runs to the first topic and each topic to the next', () => {
  const split = splitGuide(guide, topics);
  expect(split.core).toBe('Core text about syntax.\n\nMore core.');
  expect([...split.sections.keys()]).toEqual(['one', 'two', 'three']);
  expect(split.sections.get('one')).toBe('First topic: alpha.\n\nStill alpha.');
  expect(split.sections.get('two')).toBe('Second topic: beta.');
  expect(split.sections.get('three')).toBe('Third topic: gamma.');
  expect(split.missing).toEqual([]);
});

test('nothing is lost: core plus every section holds every paragraph of the guide', () => {
  const split = splitGuide(guide, topics);
  const paragraphs = (text: string) => text.split(/\n\s*\n/).map(p => p.trim()).filter(Boolean).sort();
  expect(paragraphs(topicText(split, 'all'))).toEqual(paragraphs(guide));
});

test('the closing how-to-work paragraph stays in the core, not in the last topic', () => {
  const withTail = `${guide}\nHow to work: call get_text first.\n`;
  const split = splitGuide(withTail, topics);
  expect(split.core.endsWith('How to work: call get_text first.')).toBe(true);
  expect(split.sections.get('three')).toBe('Third topic: gamma.');
  expect(coreWithMenu(split)).toContain('How to work: call get_text first.');
});

test('a topic whose opening words are absent is reported and its text stays in the previous part', () => {
  const split = splitGuide(guide.replace('Second topic', 'Renamed'), topics);
  expect(split.missing).toEqual(['two']);
  expect(split.sections.get('one')).toContain('Renamed: beta.');
  expect([...split.sections.keys()]).toEqual(['one', 'three']);
});

test('the menu lists every topic and says how to ask', () => {
  const text = coreWithMenu(splitGuide(guide, topics));
  expect(text.startsWith('Core text about syntax.')).toBe(true);
  expect(text).toContain('get_guide with topic');
  expect(text).toContain('- one: the first');
  expect(text).toContain('- three: the third');
  expect(text).not.toContain('Still alpha');
});

test('an unknown topic lists the real ones; a guide with no topics is returned whole', () => {
  const split = splitGuide(guide, topics);
  expect(topicText(split, 'nope')).toContain('Topics: one, two, three, all.');
  const plain = splitGuide('Only a core.', topics);
  expect(plain.sections.size).toBe(0);
  expect(coreWithMenu(plain)).toBe('Only a core.');
});

test('the real topic table has unique ids and distinct opening words', () => {
  expect(new Set(TOPICS.map((t: { id: string }) => t.id)).size).toBe(TOPICS.length);
  expect(new Set(TOPICS.map((t: { start: string }) => t.start)).size).toBe(TOPICS.length);
});
