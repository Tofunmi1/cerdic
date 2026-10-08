import { applyCommands } from "./commands";
import type { SceneCommand, SceneDocument } from "./commands";

export interface History {
  past: SceneDocument[];
  present: SceneDocument;
  future: SceneDocument[];
  revision: number;
}
export const createHistory = (scene: SceneDocument): History => ({
  past: [],
  present: scene,
  future: [],
  revision: 0,
});
export function commit(history: History, commands: SceneCommand[]): History {
  const present = applyCommands(history.present, commands);
  if (JSON.stringify(present) === JSON.stringify(history.present))
    return history;
  return {
    past: [...history.past, history.present].slice(-100),
    present,
    future: [],
    revision: history.revision + 1,
  };
}
export function undo(history: History): History {
  if (!history.past.length) return history;
  return {
    past: history.past.slice(0, -1),
    present: history.past.at(-1)!,
    future: [history.present, ...history.future],
    revision: history.revision + 1,
  };
}
export function redo(history: History): History {
  if (!history.future.length) return history;
  return {
    past: [...history.past, history.present],
    present: history.future[0],
    future: history.future.slice(1),
    revision: history.revision + 1,
  };
}
