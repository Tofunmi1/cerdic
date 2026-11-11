import type { Project } from './projects';
export type WorldCheckpoint = { projectId: string; worldId: string; text: string; revision: number; updatedAt: number };
export interface CheckpointStorage { getItem(key: string): string | null; setItem(key: string, value: string): void }
const keyOf = (projectId: string, worldId: string) => `cerdic-world-checkpoint:${projectId}:${worldId}`;
export const projectWorldId = (project: Project) => project.worldId ?? `${project.id}-world`;
export function readCheckpoint(storage: CheckpointStorage, project: Project): WorldCheckpoint | undefined {
  try {
    const value = JSON.parse(storage.getItem(keyOf(project.id, projectWorldId(project))) ?? 'null') as WorldCheckpoint | null;
    if (value?.projectId === project.id && value.worldId === projectWorldId(project) && typeof value.text === 'string'
      && Number.isSafeInteger(value.revision) && value.revision >= 0 && Number.isFinite(value.updatedAt)) return value;
  } catch { /* Corrupt recovery data must not prevent opening the saved project. */ }
  return undefined;
}
/** Synchronous checkpoint of committed text: available even if reload interrupts IndexedDB. */
export function checkpointWorld(storage: CheckpointStorage, project: Project, text: string, revision: number, now = Date.now()): WorldCheckpoint {
  const checkpoint = { projectId: project.id, worldId: projectWorldId(project), text, revision, updatedAt: now };
  storage.setItem(keyOf(project.id, checkpoint.worldId), JSON.stringify(checkpoint));
  return checkpoint;
}
export function recoverCheckpoint(storage: CheckpointStorage, project: Project): Project {
  const checkpoint = readCheckpoint(storage, project);
  return checkpoint && checkpoint.updatedAt >= project.updatedAt && checkpoint.text !== project.text
    ? { ...project, text: checkpoint.text, updatedAt: checkpoint.updatedAt } : project;
}
