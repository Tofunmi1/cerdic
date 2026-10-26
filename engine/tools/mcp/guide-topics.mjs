// The language guide split into a short core and topics a model asks for when it needs them (progressive disclosure):
// the whole guide is about 45 KB, and a session that builds a vase has no use for the rigging chapter.
//
// The guide is plain paragraphs, so topics are found by the opening words of their first paragraph; a topic runs until
// the next topic starts. Nothing is edited or duplicated: core + every section, in order, is the whole guide.

export const TOPICS = [
  { id: 'curves', start: 'Curved paths (initial', summary: 'paths, helices, fillets, sweeps and lofts' },
  { id: 'terrain', start: 'Landscapes:', summary: 'terrain heightfields, water and atmosphere' },
  { id: 'poly', start: 'Polygon editing (faces', summary: 'editing faces, edges and vertices: select, extrude, inset, bevel, cut, mirror, spin, UVs, LOD' },
  { id: 'groups', start: 'Groups (build once', summary: 'groups and kits, instances, rigid alignment and assembly sockets' },
  { id: 'look', start: 'Colour and finish', summary: 'materials, styles, procedural textures' },
  { id: 'animation', start: 'Animation:', summary: 'tracks, waves, clips, colour and visibility animation' },
  { id: 'scripts', start: 'Scripts (behaviour you write)', summary: 'behaviour in JavaScript: the script tool, the script.js node, the world tool, batching' },
  { id: 'rigging', start: 'Bendy things', summary: 'bones, skin weights and inverse kinematics' },
  { id: 'behaviour', start: 'Assemblies:', summary: 'parenting, behaviours and physics: parts that react in play mode' },
];

/** The closing paragraph on how to work (read the document, apply text, read the result) belongs to every session. */
export const TAIL = 'How to work:';

/**
 * Splits `guide` at the first paragraph of each topic. Returns the core (everything before the first topic, plus the
 * closing `tail` paragraph), the sections by id, and the ids whose opening words were not found (those are left inside
 * the part before them so nothing is lost).
 */
export function splitGuide(guide, topics = TOPICS, tail = TAIL) {
  const tailAt = guide.lastIndexOf(tail);
  const body = tailAt < 0 ? guide : guide.slice(0, tailAt);
  const ending = tailAt < 0 ? '' : guide.slice(tailAt).trim();
  const found = [];
  const missing = [];
  let from = 0;
  for (const topic of topics) {
    const at = body.indexOf(topic.start, from);
    if (at < 0) missing.push(topic.id);
    else { found.push({ ...topic, at }); from = at + 1; }
  }
  const sections = new Map();
  found.forEach((topic, i) => sections.set(topic.id, body.slice(topic.at, found[i + 1]?.at ?? body.length).trimEnd()));
  const opening = body.slice(0, found[0]?.at ?? body.length).trimEnd();
  return { core: ending ? `${opening}\n\n${ending}` : opening, sections, missing, topics: found };
}

/** What `get_guide` returns with no topic: the core, then a menu of the topics and how to ask for one. */
export function coreWithMenu(split) {
  if (split.sections.size === 0) return split.core;
  const menu = split.topics.map(topic => `- ${topic.id}: ${topic.summary}`).join('\n');
  return `${split.core}\n\nMore guidance: call get_guide with topics=["core", ...] and operators={query, family, ids, detail} to read topics and operator docs together. topic still selects one; topic="all" returns everything.\n- core: language and workflow\n${menu}\n\nFast start:\n1. Call get_guide once with the topics and operators you need; begin with start_from for a matching template and edit its working skeleton.\n2. Write programs with pipes and loops (topic="compact"); batch a known sequence of calls, or use run for loops and conditions, returning only what you need.\n3. Look with ONE snapshot of several cameras, labelled by angle.\n4. Use edit by label for small changes; run check before finishing and repair any problems.`;
}

/** The text for one topic, or an explanation listing the topics when the name is unknown. */
export function topicText(split, topic) {
  if (topic === 'core') return coreWithMenu(split);
  if (topic === 'all') return [split.core, ...split.sections.values()].join('\n\n');
  const text = split.sections.get(topic);
  if (text !== undefined) return text;
  return `There is no guide topic "${topic}". Topics: core, ${[...split.sections.keys()].join(', ')}, all.`;
}
