// When a running game owns the view, the studio's own camera controls step aside: the wheel does not zoom, a click does not grab the pointer
// (the cursor is how the player presses a menu button, and a fixed-angle camera has nothing to look around with), and a viewer hotkey is
// skipped when the game binds the same key (R reloads in a shooter; it must not also reset the view). Pure parsing, tested without a browser.
import { parseTable } from './actionMap';

export type GameCamera = { name: string; mode: string; orbit: boolean };

/** The active camera of the running game, from `game_status()`, or null when there is no game or it declares no camera. */
export function gameCamera(statusJson: string): GameCamera | null {
  try {
    const status = JSON.parse(statusJson) as { present?: boolean; camera?: GameCamera | null };
    return status.present && status.camera ? status.camera : null;
  } catch {
    return null;
  }
}

/** Whether a game is loaded, from `game_status()`. */
export function gameRunning(statusJson: string): boolean {
  try {
    return (JSON.parse(statusJson) as { present?: boolean }).present === true;
  } catch {
    return false;
  }
}

/** Every key the loaded game binds, from `game_input_map()`. */
export function boundKeys(tableJson: string): Set<string> {
  return new Set(parseTable(tableJson).flatMap(row => row.keys));
}

/** Whether a studio viewer hotkey should act: not when a running game has bound that key. */
export function viewerHotkeyWanted(code: string, keys: Set<string>, gameRunning: boolean): boolean {
  return !(gameRunning && keys.has(code));
}
