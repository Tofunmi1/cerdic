// Plays the sound effects a running game asks for (ADR 0062: effects are returned by the rules core, never raised by a script).
// The runtime numbers each effect and says where it happened; this module plays the `sfx` ones through the procedural sound
// generator (`audio_sfx`) at that place. The browser only lets sound start after a click or key press, so the first one unlocks it;
// every request is also logged in `played` whether or not sound is allowed, which is what tests and tools look at.
import type { Runtime } from './bridge';

export type PlayedEffect = { seq: number; name: string; position: [number, number, number] | null };
type Effect = { seq: number; kind: string; name: string; target: number | null; position: [number, number, number] | null };

const MAX_LOG = 200;

export function attachGameAudio(runtime: Runtime) {
  const played: PlayedEffect[] = [];
  let last = 0;
  let context: AudioContext | null = null;
  const unlock = () => {
    try { context ??= new AudioContext(); void context.resume(); } catch { context = null; }
  };
  for (const type of ['pointerdown', 'keydown']) window.addEventListener(type, unlock, { once: true });

  const listen = () => {
    const raw = context && runtime.viewer_pose?.();
    if (!context || !raw) return;
    const { position: p, rotation: [x, y, z, w] } = JSON.parse(raw) as { position: number[]; rotation: number[] };
    const l = context.listener;
    l.positionX.value = p[0]; l.positionY.value = p[1]; l.positionZ.value = p[2];
    l.forwardX.value = -(2 * (x * z + w * y)); l.forwardY.value = -(2 * (y * z - w * x)); l.forwardZ.value = -(1 - 2 * (x * x + y * y));
    l.upX.value = 2 * (x * y - w * z); l.upY.value = 1 - 2 * (x * x + z * z); l.upZ.value = 2 * (y * z + w * x);
  };
  const play = (effect: Effect) => {
    if (!context || context.state !== 'running') return;
    const samples = runtime.audio_sfx(effect.name, effect.seq, 1);
    if (!samples.length) return;
    listen();
    const buffer = context.createBuffer(1, samples.length, 48000);
    buffer.copyToChannel(new Float32Array(samples), 0);
    const source = context.createBufferSource();
    source.buffer = buffer;
    const gain = context.createGain();
    gain.gain.value = 0.65;
    const panner = context.createPanner();
    panner.panningModel = 'HRTF';
    const at = effect.position ?? [context.listener.positionX.value, context.listener.positionY.value, context.listener.positionZ.value];
    panner.positionX.value = at[0]; panner.positionY.value = at[1]; panner.positionZ.value = at[2];
    source.connect(gain).connect(panner).connect(context.destination);
    source.onended = () => { source.disconnect(); gain.disconnect(); panner.disconnect(); };
    source.start();
  };

  let stopped = false;
  const tick = () => {
    if (stopped) return;
    try {
      for (const effect of JSON.parse(runtime.game_effects_since(last)) as Effect[]) {
        last = Math.max(last, effect.seq);
        if (effect.kind !== 'sfx') continue;
        played.push({ seq: effect.seq, name: effect.name, position: effect.position });
        if (played.length > MAX_LOG) played.shift();
        play(effect);
      }
    } catch { /* no game or no runtime yet */ }
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
  return { played, stop() { stopped = true; } };
}
