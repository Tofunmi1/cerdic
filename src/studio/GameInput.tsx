// Keyboard, mouse and gamepad for games that read input in their scripts (`input.axis`, `input.pressed`). The page only names things: it sets
// the axes `mx`, `mz` (move), `lx`, `ly` (look, pixels a second), `block` (held), and presses `light`, `heavy`, `dodge`, `block`, `spell`,
// `special`, `jump`; what they mean is the game's own business. A game that never reads them is not affected. Inputs a game declares
// (`input NAME ... keys= pad=`) are driven from the action map in ./actionMap.
import { useEffect } from 'react';
import { host } from '../engine/host';
import { rt } from './engine-api';
import { type Row, UI_ROWS, edges, hitButton, labelsFor, loadOverrides, parseTable, resolve, withDefaults } from './actionMap';
import { uiButtons } from './uiButtons';

type Runtime = { game_input_axis?(name: string, value: number): void; game_input_press?(name: string): void; game_input_map?(): string; game_ui_labels?(json: string): void };

const typing = (target: EventTarget | null) => target instanceof HTMLElement && (target.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName));

export function GameInput() {
  useEffect(() => {
    const down = new Set<string>();
    const overrides = loadOverrides();
    let tableText = '', table: Row[] = [], heldBefore: Record<string, boolean> = {};
    let device: 'keyboard' | 'pad' = 'keyboard', labelled = '';
    let lookX = 0, lookY = 0, last = performance.now(), frame = 0;
    const runtime = () => rt() as unknown as Runtime | null;
    const press = (name: string) => runtime()?.game_input_press?.(name);
    const locked = () => document.pointerLockElement === host.canvas;

    // A press is sent from the key event itself, not only noticed by the once-a-frame poll: a tap shorter than a frame (easy on a slow frame)
    // would otherwise be missed. `byEvent` remembers what was already sent so the poll does not send it a second time.
    let rows: Row[] = [...UI_ROWS];
    const byEvent = new Set<string>();
    const pressedBy = (code: string) => {
      for (const row of rows) {
        if (row.kind === 'press' && row.keys.includes(code) && !byEvent.has(row.name)) { byEvent.add(row.name); press(row.name); }
      }
    };

    const keyDown = (event: KeyboardEvent) => {
      if (typing(event.target) || event.repeat || event.ctrlKey || event.metaKey || event.altKey) return;
      down.add(event.code);
      pressedBy(event.code);
      device = 'keyboard';
      const map: Record<string, string> = { Space: 'dodge', KeyJ: 'light', KeyK: 'heavy', KeyQ: 'spell', KeyE: 'special', ShiftLeft: 'block', ShiftRight: 'block', KeyC: 'jump' };
      if (map[event.code]) press(map[event.code]);
    };
    const keyUp = (event: KeyboardEvent) => { down.delete(event.code); };
    const mouseMove = (event: MouseEvent) => { if (locked()) { lookX += event.movementX; lookY += event.movementY; } };
    const mouseDown = (event: MouseEvent) => {
      down.add(`Mouse${event.button}`); // a game can bind a mouse button (keys="Mouse0") whether or not the pointer is captured
      pressedBy(`Mouse${event.button}`);
      if (!locked()) {
        // a click on one of the game's UI buttons presses it
        const rect = host.canvas.getBoundingClientRect();
        const id = rect.height ? hitButton(uiButtons.current, event.clientX - rect.left, event.clientY - rect.top, rect.height) : undefined;
        if (id) press(`ui:${id}`);
        return;
      }
      if (event.button === 0) press('light'); else if (event.button === 2) press('heavy');
      down.add(`Mouse${event.button}`);
    };
    const mouseUp = (event: MouseEvent) => { down.delete(`Mouse${event.button}`); };
    const blur = () => down.clear();

    const send = (now: number) => {
      frame = requestAnimationFrame(send);
      const api = runtime();
      if (!api?.game_input_axis) return;
      const dt = Math.max(0.001, (now - last) / 1000);
      last = now;
      const axis = (positive: string[], negative: string[]) => (positive.some(k => down.has(k)) ? 1 : 0) - (negative.some(k => down.has(k)) ? 1 : 0);
      let mx = axis(['KeyD'], ['KeyA']), mz = axis(['KeyW', 'ArrowUp'], ['KeyS', 'ArrowDown']);
      let turn = axis(['ArrowRight'], ['ArrowLeft']) * 700;
      let block = down.has('ShiftLeft') || down.has('ShiftRight') ? 1 : 0;
      const pad = navigator.getGamepads?.().find(p => p?.connected);
      if (pad) {
        const dead = (v: number) => (Math.abs(v) < 0.18 ? 0 : v);
        mx = mx || dead(pad.axes[0] ?? 0); mz = mz || -dead(pad.axes[1] ?? 0);
        turn = turn || dead(pad.axes[2] ?? 0) * 900;
        lookY += dead(pad.axes[3] ?? 0) * 900 * dt;
        block = block || (pad.buttons[6]?.value ?? 0) > 0.5 ? 1 : 0;
        const edge = (index: number, ...names: string[]) => { const now = !!pad.buttons[index]?.pressed; if (now && !padState[index]) names.forEach(press); padState[index] = now; };
        edge(0, 'dodge'); edge(2, 'light'); edge(3, 'heavy'); edge(1, 'spell'); edge(5, 'special');
      }
      api.game_input_axis('mx', mx); api.game_input_axis('mz', mz);
      // The inputs the game declared (with their keys and pad controls, the player's rebinding applied): axes NAME.AXIS, a held axis NAME for
      // a press input, and one press on each rise. The legacy names above stay for games that read those.
      const text = api.game_input_map?.() ?? '';
      if (text !== tableText) { tableText = text; table = parseTable(text); }
      rows = [...withDefaults(table, overrides), ...UI_ROWS];
      const state = resolve(rows, { down, pad: pad ? { axes: [...pad.axes], buttons: [...pad.buttons] } : null });
      if (pad && pad.buttons.some(b => b.pressed)) device = 'pad';
      // prompts name the keys of the device the player is using; sent when that or the game's inputs change
      const labels = JSON.stringify(labelsFor(rows, device));
      if (labels !== labelled) { labelled = labels; api.game_ui_labels?.(labels); }
      for (const [name, value] of Object.entries(state.axes)) api.game_input_axis(name, value);
      edges(heldBefore, state.held).filter(name => !byEvent.has(name)).forEach(press);
      for (const name of [...byEvent]) if (!state.held[name]) byEvent.delete(name); // released (or never seen held): ready for the next press
      heldBefore = state.held;
      api.game_input_axis('lx', lookX / dt + turn); api.game_input_axis('ly', lookY / dt);
      api.game_input_axis('block', block);
      lookX = 0; lookY = 0;
    };
    const padState: Record<number, boolean> = {};
    frame = requestAnimationFrame(send);
    window.addEventListener('keydown', keyDown); window.addEventListener('keyup', keyUp);
    window.addEventListener('mousemove', mouseMove); window.addEventListener('mousedown', mouseDown); window.addEventListener('mouseup', mouseUp);
    window.addEventListener('blur', blur);
    const noMenu = (event: MouseEvent) => { if (locked()) event.preventDefault(); };
    window.addEventListener('contextmenu', noMenu);
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener('keydown', keyDown); window.removeEventListener('keyup', keyUp);
      window.removeEventListener('mousemove', mouseMove); window.removeEventListener('mousedown', mouseDown); window.removeEventListener('mouseup', mouseUp);
      window.removeEventListener('blur', blur); window.removeEventListener('contextmenu', noMenu);
    };
  }, []);
  return null;
}
