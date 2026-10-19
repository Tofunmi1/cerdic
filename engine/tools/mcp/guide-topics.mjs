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
  return `${split.core}\n\nMore guidance by topic: call get_guide with topic set to one of these (topic="all" returns everything):\n${menu}`;
}

/** The text for one topic, or an explanation listing the topics when the name is unknown. */
export function topicText(split, topic) {
  if (topic === 'all') return [split.core, ...split.sections.values()].join('\n\n');
  const text = split.sections.get(topic);
  if (text !== undefined) return text;
  return `There is no guide topic "${topic}". Topics: ${[...split.sections.keys()].join(', ')}, all.`;
}
