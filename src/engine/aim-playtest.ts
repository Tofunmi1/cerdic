import type { Runtime } from './bridge';
import { nextScale, planShot, swingFrames } from '../../engine/web/aim-policy.js';
import type { AimPolicy, Position, ProfilePoint, ShotPlan } from '../../engine/web/aim-policy.js';

type Entity = { id: number; kind: string; position: Position; tags?: string[] };
type GameStatus = { present: boolean; status: string; vars: Record<string, number>; entities: Entity[] };
const sleep = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
const frame = () => new Promise<void>(resolve => requestAnimationFrame(() => resolve()));

/** Exercise the live game's rolling and swing rules through actual frame-by-frame hand poses. */
export async function playAimPolicy(runtime: Runtime, policy: AimPolicy, gameText: string) {
  // game_status identifies kinds; canonical game text supplies their tags.
  const tags = new Map<string, string[]>();
  for (const line of gameText.split('\n')) {
    const kind = /^kind\s+(\S+)/.exec(line.trim());
    if (kind) tags.set(kind[1], /\btags=([^\s]+)/.exec(line)?.[1].split(',') ?? []);
  }
  const status = (): GameStatus => JSON.parse(runtime.game_status());
  const entity = (s: GameStatus, tag: string) => s.entities.find(e => e.tags?.includes(tag) || tags.get(e.kind)?.includes(tag) || e.kind === tag);
  const progress = (s: GameStatus) => JSON.stringify(['hole', 'stage', 'level', 'round', 'target', 'completed'].map(key => s.vars[key]));
  const advanced = (before: GameStatus, after: GameStatus, target: string) =>
    after.status !== 'playing' || progress(after) !== progress(before) || !entity(after, target);
  const setHand = (position: Position) => {
    if (!runtime.game_set_hands(JSON.stringify([{ side: 'right', position, grip: false, trigger: false, rotation: [0, 0, 0, 1] }]))) {
      throw new Error('the runtime refused a hand pose');
    }
  };
  const groundProfile = async (ball: Position, cup: Position): Promise<ProfilePoint[]> => {
    const id = runtime.world_call('ground.profile', JSON.stringify({ from: [ball[0], ball[2]], to: [cup[0], cup[2]], samples: 200 }));
    if (!id) throw new Error('ground.profile request queue is full');
    for (let i = 0; i < 200; i++) {
      const raw = runtime.world_take(id);
      if (raw) {
        const reply = JSON.parse(raw) as { ok: boolean; result?: { points: ProfilePoint[] } };
        if (!reply.ok || !reply.result?.points?.length) throw new Error(`ground.profile: ${raw}`);
        return reply.result.points;
      }
      await sleep(30);
    }
    throw new Error('ground.profile did not answer within 6 seconds');
  };
  const rest = async (before: GameStatus, target: string) => {
    let last: Position | undefined, still = 0;
    for (let i = 0; i < 160; i++) {
      await sleep(250);
      const s = status(), ball = entity(s, policy.thing)?.position;
      if (advanced(before, s, target) || !ball) return s;
      if (last && Math.hypot(ball[0] - last[0], ball[2] - last[2]) < 0.002) {
        if (++still >= 5) return s;
      } else still = 0;
      last = ball;
    }
    return status();
  };

  const strokesPerTarget: Array<{ target: string; strokes: number; shots: number; advanced: boolean }> = [];
  let shotsTaken = 0, lastPlan: (ShotPlan & { scale: number }) | null = null;
  let error: string | undefined;
  try {
    for (const target of policy.targets ?? [policy.target]) {
      const before = status();
      if (before.status !== 'playing') break;
      let scale = 1, shots = 0, done = false;
      try {
        for (; shots < 14 && !done;) {
          const s = status();
          const ball = entity(s, policy.thing)?.position, cup = entity(s, target)?.position;
          if (!ball || !cup) throw new Error(`aim needs live entities tagged ${policy.thing} and ${target}`);
          const plan = planShot({ ball, cup, profile: await groundProfile(ball, cup) });
          lastPlan = { ...plan, speed: Math.max(0.5, Math.min(16, plan.speed * scale)), scale };
          shots++;
          shotsTaken++;
          for (const position of swingFrames({ ball, dir: plan.dir, speed: lastPlan.speed })) {
            setHand(position);
            await frame();
          }
          // Park away from the thing before waiting, as in minigolf-verify.
          setHand([ball[0], 1.2, ball[2] + 2]);
          for (let i = 0; i < 4; i++) await frame();
          const after = await rest(s, target);
          done = advanced(s, after, target);
          const end = entity(after, policy.thing)?.position;
          if (!done && end) scale = nextScale(scale, { start: ball, ball: end, cup });
        }
      } finally {
        const after = status();
        const strokes = Number.isFinite(before.vars.total) && Number.isFinite(after.vars.total) ? after.vars.total - before.vars.total : shots;
        strokesPerTarget.push({ target, strokes, shots, advanced: done });
      }
      if (!done) break;
    }
  } catch (cause) {
    error = cause instanceof Error ? cause.message : String(cause);
  } finally {
    runtime.game_set_hands('');
  }
  const final = status();
  return { status: final.status, vars: final.vars, strokesPerTarget, shotsTaken, lastPlan, won: final.status === 'won', ...(error ? { error } : {}) };
}
