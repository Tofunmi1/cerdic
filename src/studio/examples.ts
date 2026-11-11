// The example worlds shown on the start screen. They are the demo programs in engine/web/demos, bundled as text; each starts with a few `#` comment lines
// (`# about:` says what it is, `# kind:` is model, world or game), which become the card's description and tag.
const files = import.meta.glob('../../engine/web/demos/*.txt', { query: '?raw', import: 'default', eager: true }) as Record<string, string>;

export type Example = { id: string; title: string; about: string; kind: string; text: string };

const titleOf = (id: string) => id.replace(/[-_]+/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase());

// Test fixtures and swatch sheets: useful to the engine, not the first thing to show a person.
const HIDDEN = /^(materials_[a-z]|materials_grid|animation|physics|physics_big|physics_joints|physics_walk|building_asset|scenery)$/;

function parse(path: string, text: string): Example {
  const id = path.replace(/^.*\//, '').replace(/\.txt$/, '');
  const header = text.split('\n').slice(0, 12).filter((line) => line.startsWith('#'));
  const field = (name: string) => header.find((line) => line.toLowerCase().startsWith(`# ${name}:`))?.slice(name.length + 3).trim();
  const about = field('about') ?? header[0]?.replace(/^#\s*/, '') ?? '';
  return { id, title: titleOf(id), about: about.replace(/\s+/g, ' ').slice(0, 200), kind: field('kind') ?? 'model', text };
}

export const EXAMPLES: Example[] = Object.entries(files)
  .map(([path, text]) => parse(path, text))
  .filter((example) => !HIDDEN.test(example.id) && example.text.trim().length > 0)
  .sort((a, b) => a.title.localeCompare(b.title));
