import { ArrowDown, ArrowRight, ArrowUpRight, Boxes, Check, Moon, MoveUpRight, Sun, SunMoon } from 'lucide-react';
import { useEffect, useState } from 'react';
import '@fontsource-variable/jetbrains-mono';
// The studio's own stylesheet supplies the tokens, so the page and the app share one look.
import '../studio/app.css';
import './landing.css';
import { useTheme } from '../studio/hooks';
import { Boundary } from './Boundary';
import { OperatorExplorer } from './OperatorExplorer';
import { Stats } from './Stats';
import { useScrollProgress, useSpotlight } from './effects';

const studioExample = (example: string) => `/studio?example=${encodeURIComponent(example)}&docname=${encodeURIComponent(example)}`;
const studioWorld = (world: string, name: string) => `/studio?world=${encodeURIComponent(world)}&docname=${encodeURIComponent(name)}`;
const JAPAN_AT_DUSK = studioWorld('worlds/japan_village.txt', 'japan_village');

const PROJECTS = [
  { id: 'paris-boulevard', image: 'paris-boulevard.webp', title: 'Paris, in miniature', category: 'WORLD BUILDING', detail: 'Eiffel Tower · 28k triangles of ironwork' },
  { id: 'japanese_village_set', image: 'japan-shrine.webp', title: 'A quiet mountain shrine', category: 'ENVIRONMENTS', detail: 'Machiya · pond · torii · pagoda' },
  { id: 'shift-house-abstract-4story', image: 'shift-house.webp', title: 'The Shift House', category: 'ARCHITECTURE', detail: '80 nodes · 24 parts · 1,056 triangles' },
];

// Every operator family and how many operators it has, counted from the registry (263 in all, 38 families).
const FAMILIES: [string, number][] = [['poly', 60], ['game', 25], ['field', 25], ['solid', 23], ['xform', 16], ['inst', 13], ['curve', 12], ['terrain', 8], ['body', 8], ['rig', 7], ['profile', 7], ['anim', 7],
  ['volume', 5], ['value', 5], ['fx', 5], ['phys', 4], ['light', 3], ['check', 3], ['water', 2], ['street', 2], ['scene', 2], ['river', 2], ['group', 2], ['cloth', 2], ['city', 2],
  ['sim', 1], ['scatter', 1], ['rooms', 1], ['material', 1], ['fluid', 1], ['flora', 1], ['building', 1], ['bake', 1], ['behavior', 1], ['script', 1], ['frame', 1], ['detail', 1], ['sign', 1]];
const COUNT = new Map(FAMILIES);
const TOTAL = FAMILIES.reduce((sum, [, n]) => sum + n, 0);
/** The operators in the families a card names, so the number on the card is always the registry's. */
const opsOf = (families: string[]) => families.reduce((sum, name) => sum + (COUNT.get(name) ?? 0), 0);

// Everything the engine does, by area. Each card names the operator families behind it; the count is worked out from them.
const CAPABILITIES = [
  { title: 'Solids, curves and fields', text: 'Primitives, lofts, sweeps and revolves, rounded edges, robust booleans and smooth-union fields for organic forms. Curves with fillet, resample and trim.', families: ['solid', 'profile', 'curve', 'field', 'volume'] },
  { title: 'Polygon editing', text: 'Bevel, inset, extrude, solidify, poke, wireframe, dissolve, mirror, decimate for LOD, unsubdivide, UV unwrap with island packing and quad remeshing.', families: ['poly'] },
  { title: 'Terrain and worlds', text: 'Tiled terrain with erosion, rivers and lakes, rocks, trees and plants placed by rule, water with waves, streets, buildings, room plans and whole districts.', families: ['terrain', 'river', 'water', 'flora', 'scatter', 'city', 'street', 'building', 'rooms'] },
  { title: 'Sky, light and rendering', text: 'Physically based sky, atmosphere and clouds, haze, shadows, lights that come from emissive surfaces, baked light probes, depth of field and motion blur.', families: ['scene', 'light', 'frame'] },
  { title: 'Materials and textures', text: '398 procedural texture nodes baked to PBR maps in the browser. Shape-aware weathering, glass, foliage with wind, clear coat and translucency, plus recipes for wood, brick, marble and fabric.', families: ['material', 'bake'] },
  { title: 'Characters and creatures', text: 'A human body built from an open CC0 dataset with clothes and hair, procedural quadrupeds and creatures, rigs with IK, foot placement for any number of legs, ragdolls.', families: ['body', 'rig'] },
  { title: 'Animation', text: 'Keyframed tracks with eased, bezier, bounce and elastic curves; parents that carry children; state machines, layered clips, retargeting, and behaviours on click or proximity.', families: ['anim', 'behavior'] },
  { title: 'Physics, effects and simulation', text: 'Rigid bodies, joints and vehicles; particles and fire with sub-emitters and trails; cloth, fluid and water simulation; programs that run every frame; procedural audio.', families: ['phys', 'fx', 'cloth', 'fluid', 'sim'] },
  { title: 'Games', text: 'Rules, goals, inventories, HUD and save slots; characters with their own behaviour; navigation meshes with A* and path smoothing; a playtest tool that plays the game it just made.', families: ['game'] },
  { title: 'Placing and reusing', text: 'Move, rotate, scale and mirror; instance sets with per-copy variation; named groups and reusable kits; numbers, expressions and a small permissioned script node.', families: ['xform', 'inst', 'group', 'value', 'script'] },
  { title: 'Checking and shipping', text: 'Collision and measurement checks with typed diagnostics, snapshots from any camera, and export to .glb with textures, instancing and animation, or to plain .txt.', families: ['check'] },
];

const STEPS = [
  ['01', 'Set the brief', 'Describe the world and give it real constraints: dimensions, parts, style and intent.'],
  ['02', 'Build in the open', 'The agent edits a readable scene program. The same scene stays available in the browser workspace.'],
  ['03', 'Measure, then refine', 'Inspect geometry, run checks and capture fixed views. Revise the named part that needs work.'],
];

export default function Landing() {
  const { theme, next } = useTheme();
  const ThemeIcon = theme === 'auto' ? SunMoon : theme === 'dark' ? Moon : Sun;
  const [activeFamily, setActiveFamily] = useState<string | null>(null);
  useSpotlight();
  useScrollProgress();
  useEffect(() => {
    const scrollRoot = document.querySelector<HTMLElement>('.landing');
    const targets = document.querySelectorAll<HTMLElement>('.landing [data-reveal]');
    if (!scrollRoot || window.matchMedia('(prefers-reduced-motion: reduce)').matches || !('IntersectionObserver' in window)) {
      targets.forEach((target) => target.classList.add('is-visible'));
      return;
    }
    const observer = new IntersectionObserver((entries) => {
      entries.forEach((entry) => {
        if (entry.isIntersecting) {
          entry.target.classList.add('is-visible');
          observer.unobserve(entry.target);
        }
      });
    }, { root: scrollRoot, rootMargin: '0px 0px -48px 0px', threshold: 0.08 });
    targets.forEach((target, index) => {
      target.style.setProperty('--reveal-delay', `${Math.min(index % 4, 3) * 65}ms`);
      observer.observe(target);
    });
    return () => observer.disconnect();
  }, []);
  return (
    <div className="landing">
      <div className="scroll-progress" aria-hidden="true" />
      <header className="landing-nav">
        <a className="landing-brand" href="/" aria-label="Cerdic 3D home"><span className="brand-mark"><Boxes size={17} strokeWidth={1.8} /></span><span>Cerdic<span className="brand-light">3D</span></span></a>
        <nav aria-label="Main navigation"><a href="#approach">Approach</a><a href="#work">Selected work</a><a href="#tools">What it can do</a></nav>
        <button className="theme-toggle" type="button" onClick={next} aria-label={`Theme: ${theme}`} title={`Theme: ${theme}`}><ThemeIcon size={16} /></button>
        <a className="nav-cta" href="/studio?projects=1">Open workspace <ArrowUpRight size={15} /></a>
      </header>

      <main>
        <section className="landing-hero">
          <div className="hero-topline"><span>AN AGENTIC 3D WORKSPACE</span><span>DESIGN · BUILD · VERIFY</span></div>
          <div className="hero-content">
            <div className="hero-message">
              <p className="hero-kicker"><span />Make the world you can describe.</p>
              <h1>From an idea<br />to <em>an editable world.</em></h1>
              <p className="hero-description">Describe a scene and a model writes it as text, one node per line. Then it measures the result, looks at it from its own cameras and fixes what is wrong. Open any line, change a number, undo it. Nothing is imported.</p>
              <div className="hero-actions"><a className="primary-cta" href={JAPAN_AT_DUSK}>Explore a real scene <ArrowRight size={16} /></a><a className="quiet-cta" href="#approach">How it works <ArrowDown size={14} /></a></div>
              <div className="hero-proof"><span><Check size={13} />Runs in your browser</span><span><Check size={13} />Editable scene source</span><span><Check size={13} />MCP tool workflow</span></div>
            </div>
            <figure className="hero-art">
              <img src="/landing/japan-aerial.jpg" alt="A mountain village at sunset: a street of townhouses leading up to a shrine between wooded hills, a scene generated from text" width="1920" height="950" fetchPriority="high" />
              <figcaption><span className="art-index">01 / 03</span><span>JAPANESE VILLAGE <b>—</b> written as text, drawn by the engine</span><a href="#work" aria-label="Explore selected work"><MoveUpRight size={15} /></a></figcaption>
              <span className="art-note">SCREENSHOT · LIVE VIEWER</span>
            </figure>
          </div>
          <div className="hero-bottomline"><span>SCENE GRAPHS, SURFACES, WORLDS</span><span>SCROLL TO EXPLORE <span className="scroll-rule" /></span></div>
        </section>

        <Boundary name="stats"><Stats /></Boundary>

        <section className="manifesto" id="approach" data-reveal>
          <div className="manifesto-label"><span>01</span><span>THE APPROACH</span></div>
          <div className="manifesto-main"><h2>Models are better builders when they can <em>measure their work.</em></h2><p>Give an agent a scene document, a set of tools and a clear target. It can build, inspect the result, and make focused changes—while you stay in control of the craft.</p></div>
          <div className="steps-list">{STEPS.map(([number, title, text]) => <article className="step" key={number}><span className="step-number">{number}</span><div><h3>{title}</h3><p>{text}</p></div><ArrowUpRight size={16} /></article>)}</div>
          <div className="tool-strip"><span>ONE TOOLCHAIN, FROM PROMPT TO PROOF</span><code>apply_text</code><code>check</code><code>measure</code><code>inspect</code><code>snapshot</code><code>edit</code></div>
        </section>

        <section className="work-section" id="work">
          <div className="section-overline"><span>02 / SCENE STUDIES</span><a href="/studio?projects=1">Open the workspace <ArrowUpRight size={14} /></a></div>
          <div className="work-heading"><h2>Worlds built<br />to be <em>worked on.</em></h2><p>These are real, editable scenes from the engine. Open one to inspect the source and keep building.</p></div>
          <div className="project-grid">{PROJECTS.map((project, index) => <a className={`project-card project-${index + 1}`} key={project.id} href={studioExample(project.id)} data-reveal>
            <div className="project-image"><img src={`/landing/${project.image}`} alt={`${project.title}, an editable scene built in Cerdic`} loading="lazy" width="1920" height="928" /><span className="project-open"><ArrowUpRight size={18} /></span><span className="project-number">0{index + 1}</span></div>
            <div className="project-meta"><span>{project.category}</span><span>{project.detail}</span></div><h3>{project.title}</h3>
          </a>)}</div>
        </section>

        <section className="tool-section" id="tools">
          <div className="tool-heading" data-reveal><div><span className="section-index">03 / WHAT IT CAN DO</span><h2>{TOTAL} operators.<br /><em>One scene language.</em></h2></div><p>Geometry, worlds, materials, characters, motion, physics, games and checks are all operators in the same text language. Whatever you build, you can also edit, measure and undo.</p></div>
          <div className="tool-categories">{CAPABILITIES.map((item, index) => (
            <article key={item.title} data-reveal className={activeFamily ? item.families.includes(activeFamily) ? 'family-match' : 'family-dimmed' : undefined}>
              <span>{String(index + 1).padStart(2, '0')} · {opsOf(item.families)} operators</span>
              <h3>{item.title}</h3>
              <p>{item.text}</p>
              <div className="cap-tags">{item.families.map((name) => <code key={name}>{name}</code>)}</div>
            </article>
          ))}</div>
          <div className="family-cloud" aria-label={`All ${FAMILIES.length} operator families`} data-reveal>
            <span className="family-cloud-label">{activeFamily ? `${activeFamily} · ${COUNT.get(activeFamily)} OPERATORS` : `ALL ${FAMILIES.length} FAMILIES · SELECT TO TRACE`}</span>
            {FAMILIES.map(([name, count]) => <button className={activeFamily === name ? 'family-selected' : undefined} key={name} type="button" aria-label={`Toggle ${name} family filter (${count} operators)`} aria-pressed={activeFamily === name} onClick={() => setActiveFamily(activeFamily === name ? null : name)} title={`Highlight ${name} operators (${count})`}><code>{name}<b>{count}</b></code></button>)}
            {activeFamily && <button className="family-clear" type="button" onClick={() => setActiveFamily(null)}>Clear filter <span aria-hidden="true">×</span></button>}
          </div>
          <Boundary name="explorer"><OperatorExplorer family={activeFamily} onFamily={setActiveFamily} /></Boundary>
          <p className="built-on" data-reveal><span>BUILT ON</span>A Rust evaluator compiled to WebAssembly · a C++ geometry kernel (Manifold booleans, OpenSubdiv subdivision, a full polygon-editing toolkit) · a trimmed Bevy renderer · 398 procedural texture nodes · an MCP server for agents. Native and WebAssembly runs of one document are compared bit for bit.</p>
          <div className="agent-callout" data-reveal><div><span>FOR AGENT BUILDERS</span><h3>Bring your own model.<br />Give it real tools.</h3><p>The MCP server exposes the scene guide, editing, geometry checks, measurements and snapshots. The browser workspace does not call an LLM by itself.</p></div><div className="agent-code"><span>THE BUILD LOOP</span><code><b>01</b> read guide <i>→</i> <b>02</b> apply scene <i>→</i></code><code><b>03</b> check + measure <i>→</i> <b>04</b> inspect + revise</code><a href="/studio?mcp=1">Open agent workspace <ArrowRight size={14} /></a></div></div>
        </section>
      </main>

      <footer className="landing-footer" data-reveal><a className="landing-brand" href="/"><span className="brand-mark"><Boxes size={16} /></span><span>Cerdic<span className="brand-light">3D</span></span></a><span>Made for worlds that are still becoming.</span><a href="/studio?projects=1">Start creating <ArrowUpRight size={15} /></a></footer>
    </div>
  );
}
