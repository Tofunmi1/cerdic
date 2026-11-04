// Realtime textures: a texture program that reads the clock (an `expr` node using `t`, a Material Maker node using `$time`) is not baked once. Its albedo shader is compiled once, then drawn on the GPU
// every frame with the current time and the picture is sent to the renderer's map for that part (`viewer_texture_update`), so the surface shows a moving pattern: water that ripples, a screen that scrolls, a
// flame, a pulse. The other channels (normal, roughness) stay as baked. Only 2D programs that need nothing from the mesh can run live; the maps are 256 by 256 so each frame costs about a quarter of a megabyte.
import { Baker, type LiveBake } from './bake';
import { compileTexture, type Compiled } from './compile';
import { parseTextureProgram } from './program';
import type { Library } from './types';

/** Side of the pictures a live texture draws and sends each frame. */
export const LIVE_SIZE = 256;

/** The compiled albedo of `source` when it is animated and needs nothing from the mesh; undefined for every other program (or one that does not parse). */
export function animatedAlbedo(library: Library, helpers: string, source: string): Compiled | undefined {
  if (!source) return undefined;
  try {
    const program = parseTextureProgram(source, library);
    const output = program.channels.albedo;
    if (!output) return undefined;
    const compiled = compileTexture(library, { nodes: program.nodes, output }, helpers);
    return compiled.animated && !compiled.needs.position && !compiled.needs.surface ? compiled : undefined;
  } catch {
    return undefined;
  }
}

/** What the live textures need of the renderer. */
export type LiveTarget = { viewer_texture_update(node: string, region: number, slot: number, pixels: Uint8Array): boolean };

type Entry = { node: string; region: number; bake: LiveBake };

export class LiveTextures {
  private readonly entries: Entry[] = [];
  private baker: Baker | undefined;
  private frameHandle = 0;
  private last = -Infinity;
  private started = 0;
  /** Pictures the renderer accepted, and how many it refused (a map of another size, or a scene that has not committed yet). */
  accepted = 0;
  refused = 0;

  constructor(private readonly target: LiveTarget, private readonly now: () => number = () => performance.now() / 1000) {}

  /** Adds one part's albedo to draw live. */
  add(node: string, region: number, compiled: Compiled, size = LIVE_SIZE): void {
    this.baker ??= new Baker(new OffscreenCanvas(1, 1));
    this.entries.push({ node, region, bake: this.baker.live(compiled, size) });
  }

  get count(): number {
    return this.entries.length;
  }

  /** Draws every live texture at `seconds` and sends the pictures; returns how many the renderer accepted. */
  tick(seconds: number): number {
    let sent = 0;
    for (const entry of this.entries) {
      const pixels = entry.bake.frame(seconds);
      if (this.target.viewer_texture_update(entry.node, entry.region, 0, pixels)) { sent++; this.accepted++; } else this.refused++;
    }
    return sent;
  }

  /** Starts drawing about `hz` times a second (on animation frames), the clock starting at zero now. */
  start(hz = 30): void {
    this.started = this.now();
    const step = () => {
      const t = this.now();
      if (t - this.last >= 1 / hz - 0.002) {
        this.last = t;
        try {
          this.tick(t - this.started);
        } catch (error) {
          console.warn('a live texture failed and was stopped:', error);
          this.stop();
          return;
        }
      }
      this.frameHandle = requestAnimationFrame(step);
    };
    this.frameHandle = requestAnimationFrame(step);
  }

  /** Stops drawing and frees the GPU objects. */
  stop(): void {
    cancelAnimationFrame(this.frameHandle);
    for (const entry of this.entries) entry.bake.dispose();
    this.entries.length = 0;
    this.baker?.dispose();
    this.baker = undefined;
  }
}
