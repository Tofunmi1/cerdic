// web/aim-policy.js: aim and swing a hand-held tool at a rolling thing toward a target (the playtest policy `aim:<thing tag>:<target tag>`).
// Pure parts only: the shot size from the ground, the next shot's scale, the hand path of one swing. The page code that drives the runtime uses these.
import { expect, test } from "bun:test";
// @ts-expect-error plain JavaScript module
import { planShot, nextScale, swingFrames, parseAimPolicy } from "../aim-policy.js";

const flat = (length: number, decel: number, samples = 11) =>
  Array.from({ length: samples }, (_, i) => ({ s: (length * i) / (samples - 1), height: 1, decel, surface_name: "green" }));

test("a flat line arrives at 1 m/s: v^2 = arrive^2 + 2 * decel * length", () => {
  const plan = planShot({ ball: [0, 1, 0], cup: [0, 1, 10], profile: flat(10, 0.5), arrive: 1 });
  expect(plan.speed).toBeCloseTo(Math.sqrt(1 + 2 * 0.5 * 10), 6);
  expect(plan.dir[0]).toBeCloseTo(0, 9);
  expect(plan.dir[1]).toBeCloseTo(1, 9);
});

test("uphill needs more speed and downhill less, by 5/7 g times the slope", () => {
  const rising = flat(10, 0.5).map((p, i) => ({ ...p, height: 1 + 0.1 * p.s }));
  const falling = flat(10, 0.5).map((p) => ({ ...p, height: 2 - 0.1 * p.s }));
  const level = planShot({ ball: [0, 1, 0], cup: [0, 2, 10], profile: flat(10, 0.5), arrive: 1 }).speed;
  const up = planShot({ ball: [0, 1, 0], cup: [0, 2, 10], profile: rising, arrive: 1 }).speed;
  const down = planShot({ ball: [0, 2, 0], cup: [0, 1, 10], profile: falling, arrive: 1 }).speed;
  expect(up).toBeGreaterThan(level);
  expect(down).toBeLessThan(level);
  expect(up * up).toBeCloseTo(1 + 2 * (0.5 + (5 / 7) * 9.8 * 0.1) * 10, 3);
});

test("the direction runs from the ball to the target in x and z", () => {
  const plan = planShot({ ball: [1, 1, 1], cup: [4, 1, 5], profile: flat(5, 0.5), arrive: 1 });
  expect(plan.dir[0]).toBeCloseTo(0.6, 9);
  expect(plan.dir[1]).toBeCloseTo(0.8, 9);
});

test("a void in the profile is counted and does not stop the plan", () => {
  const profile = flat(10, 0.5).map((p, i) => (i === 5 ? { ...p, height: null } : p));
  const plan = planShot({ ball: [0, 1, 0], cup: [0, 1, 10], profile, arrive: 1 });
  expect(plan.voids).toBe(1);
  expect(plan.speed).toBeGreaterThan(1);
});

test("the speed is never below a floor and never above the cap", () => {
  expect(planShot({ ball: [0, 1, 0], cup: [0, 1, 0.2], profile: flat(0.2, 0.01, 3), arrive: 0.1 }).speed).toBeGreaterThanOrEqual(0.5);
  expect(planShot({ ball: [0, 1, 0], cup: [0, 1, 500], profile: flat(500, 1, 51), arrive: 1, cap: 16 }).speed).toBeLessThanOrEqual(16);
});

test("the next scale: a reset by the out rule shrinks it, an overshoot shrinks it less, a short shot grows it, a good shot keeps it", () => {
  const start = [0, 1, 0];
  const cup = [0, 1, 10];
  expect(nextScale(1, { start, ball: [0, 1, 0.01], cup })).toBeCloseTo(0.7, 9);
  expect(nextScale(1, { start, ball: [0, 1, 12], cup })).toBeCloseTo(0.8, 9);
  expect(nextScale(1, { start, ball: [0, 1, 5], cup })).toBeCloseTo(1.15, 9);
  expect(nextScale(1, { start, ball: [0, 1, 9.8], cup })).toBeCloseTo(1, 9);
  expect(nextScale(0.5, { start, ball: [0, 1, 5], cup })).toBeCloseTo(0.575, 9);
});

test("a swing starts 0.6 m behind the ball, passes through it at the planned speed, and ends past it", () => {
  const frames = swingFrames({ ball: [2, 1, 3], dir: [0, 1], speed: 6, fps: 60 });
  expect(frames.length).toBeGreaterThan(15); // 8 frames held at the start, then the swing
  const first = frames[0];
  expect(first[0]).toBeCloseTo(2, 6);
  expect(first[2]).toBeCloseTo(3 + 0.9 - 0.6, 6); // the head is 0.9 m out along -Z of the hand
  const speeds = frames.slice(1).map((f, i) => Math.hypot(f[0] - frames[i][0], f[2] - frames[i][2]) * 60);
  const moving = speeds.filter((v) => v > 0.1);
  expect(Math.max(...moving)).toBeCloseTo(6, 3);
  const crossing = frames.findIndex((f) => f[2] >= 3 + 0.9);
  expect(crossing).toBeGreaterThan(0);
  expect(frames[frames.length - 1][2]).toBeGreaterThan(3 + 0.9);
});

test("parseAimPolicy reads aim:<thing>:<target> and refuses anything else", () => {
  expect(parseAimPolicy("aim:ball:cup1")).toEqual({ thing: "ball", target: "cup1" });
  expect(parseAimPolicy("aim:ball:cup1,cup2,cup3")).toEqual({ thing: "ball", targets: ["cup1", "cup2", "cup3"], target: "cup1" });
  expect(parseAimPolicy("seek:hero:pickup")).toBeNull();
  expect(parseAimPolicy("aim:ball")).toBeNull();
  expect(parseAimPolicy("aim::cup")).toBeNull();
});
