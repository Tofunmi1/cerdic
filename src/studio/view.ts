// View settings: what the quality panel changes, kept in one place so the panel, the keyboard and the engine agree. Defaults match the plain viewer
// page: high quality, sun shadows and ambient occlusion on, everything else off.
import { useCallback, useEffect, useRef, useState } from 'react';
import { host } from '../engine/host';
import { rt } from './engine-api';
import { hitButton } from './actionMap';
import { boundKeys, gameCamera, gameRunning, viewerHotkeyWanted } from './gameView';
import { uiButtons } from './uiButtons';

export type Lens = 'off' | 'fisheye' | 'headset';

/** The surface effects the viewer can stop drawing (the engine's own names): each is a layer a `solid.finish` or a texture adds to a surface. */
export const EFFECTS = [
  ['subsurface', 'Subsurface scattering', 'Light softening under skin, wax and ears'],
  ['clearcoat', 'Clear coat', 'The glossy lacquer layer'],
  ['translucency', 'Translucency', 'Light through leaves, wings and thin shells'],
  ['sheen', 'Sheen', 'Soft grazing-angle glow of velvet, fur and peach fuzz'],
  ['iridescence', 'Iridescence', 'Colour that shifts with the view angle'],
  ['relief', 'Surface relief', 'Scales, pores and wrinkles drawn finer than the mesh'],
  ['wetness', 'Wetness', 'The wet, glossy look'],
  ['anisotropy', 'Anisotropic highlights', 'Highlights stretched along brushed metal and silk'],
  ['maps', 'Texture maps', 'Baked and procedural colour, normal, roughness and glow maps'],
] as const;
export type EffectName = (typeof EFFECTS)[number][0];

/** View aids drawn over the scene (see Overlays.tsx); they are the page's own and the engine never hears of them. */
export const OVERLAYS = [
  ['bounds', 'Bounding box', 'Around the selection, or the whole model'],
  ['dimensions', 'Dimensions', 'Width × height × depth in metres'],
  ['wire', 'Wireframe', 'The mesh edges of the selection'],
  ['normals', 'Normals', 'Which way each surface faces'],
  ['problems', 'Problem boxes', 'Red boxes round nodes the engine complained about'],
  ['grid', 'Ground grid', 'A metre grid that scales with the model'],
  ['axes', 'World axes', 'X red, Y green, Z blue from the origin'],
  ['stats', 'Statistics', 'Frames a second, parts and triangles'],
] as const;
export type OverlayName = (typeof OVERLAYS)[number][0];
export type Overlays = Record<OverlayName, boolean>;
export const DEFAULT_OVERLAYS = Object.fromEntries(OVERLAYS.map(([name]) => [name, false])) as Overlays;

export type View = {
  overlays: Overlays;
  fov: number; stereo: boolean; lens: Lens; orbit: boolean;
  quality: 'low' | 'quest' | 'medium' | 'high';
  shadows: boolean; shadowSize: number; smaa: boolean; effects: Record<EffectName, boolean>;
  specularAa: boolean; ssao: boolean; bloom: boolean; autoExposure: boolean;
  exposure: number; grade: string;
};

export const GRADES = ['neutral', 'studio', 'warm', 'cool', 'mono', 'sunset', 'vivid', 'film', 'night', 'noir'];
export const DEFAULT_EFFECTS = Object.fromEntries(EFFECTS.map(([name]) => [name, true])) as Record<EffectName, boolean>;
export const DEFAULT_VIEW: View = {
  overlays: DEFAULT_OVERLAYS,
  fov: 45, stereo: false, lens: 'off', orbit: false, quality: 'high',
  shadows: true, shadowSize: 0, smaa: true, effects: DEFAULT_EFFECTS,
  specularAa: false, ssao: true, bloom: false, autoExposure: false, exposure: 1, grade: 'neutral',
};
const KEY = 'studio-view';

function load(): View {
  try {
    const saved = JSON.parse(localStorage.getItem(KEY) ?? '{}') as Partial<View>;
    return { ...DEFAULT_VIEW, ...saved, effects: { ...DEFAULT_EFFECTS, ...saved.effects }, overlays: { ...DEFAULT_OVERLAYS, ...saved.overlays } };
  } catch { return DEFAULT_VIEW; }
}

/** Sends the settings to the engine, in the order that makes each choice sensible (a quality change resets shadows, so shadows go after it). */
export function applyView(view: View, changed?: keyof View) {
  const runtime = rt();
  if (!runtime) return;
  const all = changed === undefined;
  if (all || changed === 'quality') { runtime.set_quality(view.quality); runtime.set_shadows(view.shadows); }
  if (all || changed === 'shadows') runtime.set_shadows(view.shadows);
  // A shadow map size of 0 is the tier's own; a size is only sent while shadows are on (0 would turn them off).
  if ((all || changed === 'quality' || changed === 'shadows' || changed === 'shadowSize') && view.shadows && view.shadowSize > 0) {
    runtime.set_shadow_resolution(view.shadowSize);
  }
  if (all || changed === 'smaa') runtime.set_antialiasing(view.smaa);
  if (all || changed === 'effects') for (const [name] of EFFECTS) runtime.set_effect(name, view.effects[name]);
  if (all || changed === 'specularAa') runtime.set_specular_aa(view.specularAa);
  if (all || changed === 'ssao') runtime.set_ssao(view.ssao);
  if (all || changed === 'bloom') runtime.set_bloom(view.bloom);
  if (all || changed === 'autoExposure') runtime.set_auto_exposure(view.autoExposure);
  if (all || changed === 'fov' || changed === 'stereo' || changed === 'lens') {
    runtime.viewer_configure(true, view.fov, view.stereo, 3);
    runtime.viewer_set_lens(view.lens);
  }
  if (all || changed === 'grade') runtime.set_grading_preset(view.grade);
  if (all || changed === 'exposure') {
    const current = JSON.parse(runtime.grading_status()) as { contrast: number; saturation: number; tint: number[] };
    runtime.set_grading(view.exposure, current.contrast, current.saturation, current.tint[0], current.tint[1], current.tint[2]);
  }
}

/** The view settings as React state; every change goes to the engine and is remembered. Applied once the engine is ready. */
export function useViewSettings() {
  const [view, setView] = useState<View>(load);
  const ready = host.ready;
  useEffect(() => { if (ready) applyView(view); /* the saved settings, once, when the engine starts */ // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ready]);
  const latest = useRef(view);
  const set = useCallback(<K extends keyof View>(key: K, value: View[K]) => {
    let next = { ...latest.current, [key]: value };
    // A fisheye needs a wide view to bend; a headset lens needs two eyes; a layout that makes a lens useless drops it.
    if (key === 'lens' && value === 'fisheye') next = { ...next, stereo: false, fov: Math.max(next.fov, 130) };
    if (key === 'lens' && value === 'headset') next = { ...next, stereo: true };
    if (key === 'stereo' && next.lens === (value ? 'fisheye' : 'headset')) next = { ...next, lens: 'off' };
    applyView(next, key === 'lens' || key === 'stereo' ? 'lens' : key);
    // A grade preset sets its own exposure; show it.
    if (key === 'grade') {
      const status = rt()?.grading_status();
      if (status) next = { ...next, exposure: (JSON.parse(status) as { exposure: number }).exposure };
    }
    try { localStorage.setItem(KEY, JSON.stringify(next)); } catch { /* storage may be blocked */ }
    document.body.classList.toggle('split', next.stereo);
    latest.current = next;
    setView(next);
  }, []);
  return { view, set };
}

/** The camera controls of the plain viewer, for the studio's canvas: wheel to zoom, Alt or middle drag (or orbit mode) to circle, hotkeys. */
export function useViewerControls(view: View, set: <K extends keyof View>(key: K, value: View[K]) => void, onFrame: () => void, toggleChrome: () => void, onHome?: () => void) {
  useEffect(() => {
    const canvas = host.canvas;
    const runtime = () => rt();
    // what the running game says about itself: its camera, its status and the keys it binds (the studio steps aside for these)
    const game = () => (runtime() as unknown as { game_status?(): string; game_input_map?(): string } | null);
    const wheel = (event: WheelEvent) => {
      if (document.pointerLockElement === canvas || (event.buttons & 1)) return;
      event.preventDefault(); // the wheel zooms the free viewer, or the game's camera when one has the view (the runtime decides which)
      runtime()?.viewer_zoom(event.deltaY * (event.deltaMode === 1 ? 33 : 1));
    };
    let orbiting: number | null = null;
    const wanted = (event: PointerEvent) => event.button === 1 || (event.button === 0 && (event.altKey || view.orbit));
    const down = (event: PointerEvent) => {
      if (event.target !== canvas || document.pointerLockElement === canvas || !wanted(event)) return;
      orbiting = event.pointerId; canvas.setPointerCapture?.(event.pointerId);
      event.preventDefault(); event.stopImmediatePropagation();
    };
    const move = (event: PointerEvent) => {
      if (orbiting !== event.pointerId) return;
      runtime()?.viewer_orbit(event.movementX, event.movementY);
      event.stopImmediatePropagation();
    };
    const up = (event: PointerEvent) => {
      if (orbiting !== event.pointerId) return;
      orbiting = null; event.stopImmediatePropagation();
    };
    const hand = { KeyG: false, KeyT: false };
    const sendHand = () => runtime()?.game_desktop_hand?.(hand.KeyG, hand.KeyT);
    const key = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.tagName === 'SELECT' || target.isContentEditable)) return;
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      if (event.code in hand) { if (!hand[event.code as 'KeyG' | 'KeyT']) { hand[event.code as 'KeyG' | 'KeyT'] = true; sendHand(); } return; }
      // a key the running game binds (R reloads in a shooter) belongs to the game, not to the viewer
      if (!viewerHotkeyWanted(event.code, boundKeys(game()?.game_input_map?.() ?? ''), gameRunning(game()?.game_status?.() ?? ''))) return;
      if (event.code === 'KeyF') onFrame();
      else if (event.code === 'KeyR') { if (onHome) onHome(); else runtime()?.viewer_reset(); }
      else if (event.code === 'KeyV') set('stereo', !view.stereo);
      else if (event.code === 'KeyL') set('lens', ({ off: 'fisheye', fisheye: 'headset', headset: 'off' } as const)[view.lens]);
      else if (event.code === 'KeyO') set('orbit', !view.orbit);
      else if (event.code === 'KeyH') toggleChrome();
    };
    const keyUp = (event: KeyboardEvent) => {
      if (event.code in hand) { hand[event.code as 'KeyG' | 'KeyT'] = false; sendHand(); }
    };
    // One click on the view holds the mouse for looking around (Esc lets go), as in the plain viewer; a refused lock simply leaves the cursor free.
    const click = (event: MouseEvent) => {
      // a fixed-angle game camera has nothing to look around with, and the cursor is how the player presses a menu button: no grab
      const camera = gameCamera(game()?.game_status?.() ?? '');
      if (camera && !camera.orbit) return;
      const rect = canvas.getBoundingClientRect();
      if (hitButton(uiButtons.current, event.clientX - rect.left, event.clientY - rect.top, rect.height)) return;
      if (document.pointerLockElement !== canvas && !event.altKey && !view.orbit) Promise.resolve(canvas.requestPointerLock?.()).catch(() => {});
    };
    canvas.addEventListener('click', click);
    canvas.addEventListener('wheel', wheel, { passive: false });
    window.addEventListener('pointerdown', down, true);
    window.addEventListener('pointermove', move, true);
    window.addEventListener('pointerup', up, true);
    window.addEventListener('pointercancel', up, true);
    window.addEventListener('keydown', key);
    window.addEventListener('keyup', keyUp);
    return () => {
      canvas.removeEventListener('click', click);
      canvas.removeEventListener('wheel', wheel);
      window.removeEventListener('pointerdown', down, true);
      window.removeEventListener('pointermove', move, true);
      window.removeEventListener('pointerup', up, true);
      window.removeEventListener('pointercancel', up, true);
      window.removeEventListener('keydown', key);
      window.removeEventListener('keyup', keyUp);
    };
  }, [view.orbit, view.stereo, view.lens, set, onFrame, toggleChrome, onHome]);
}
