export type Position = [number, number, number];
export type AimPolicy = { thing: string; target: string; targets?: string[] };
export type ShotPlan = { dir: [number, number]; distance: number; speed: number; voids: number; surfaces: Array<string | null> };
export type ProfilePoint = { s: number; height: number | null; decel: number; surface_name: string | null };
export function planShot(args: { ball: Position; cup: Position; profile: ProfilePoint[]; arrive?: number; cap?: number }): ShotPlan;
export function nextScale(scale: number, args: { start: Position; ball: Position; cup: Position }): number;
export function swingFrames(args: { ball: Position; dir: [number, number]; speed: number; fps?: number }): Position[];
export function parseAimPolicy(policy: unknown): AimPolicy | null;
