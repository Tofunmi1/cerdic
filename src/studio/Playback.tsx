import { useEffect, useRef, useState } from 'react';
import { Pause, Play, Square } from 'lucide-react';
import { host } from '../engine/host';
import { rt } from './engine-api';

type Clock = { playing: boolean; seconds: number; speed: number };
type Info = { duration: number; looping: boolean; clock: Clock; playable: boolean; playing: boolean; machine: boolean };

const IDLE: Info = { duration: 0, looping: false, clock: { playing: true, seconds: 0, speed: 1 }, playable: false, playing: false, machine: false };

/** Reads the engine's play mode and animation clock a few times a second: the studio keeps no copy of either. */
function useInfo(): Info {
  const [info, setInfo] = useState<Info>(IDLE);
  useEffect(() => {
    const timer = setInterval(() => {
      const runtime = rt();
      if (!runtime || !host.evaluation) return;
      const behaviors = JSON.parse(runtime.viewer_behaviors()) as { parts: unknown[]; play: boolean; clock: Clock };
      const physics = JSON.parse(runtime.viewer_physics()) as { movable: number; ragdolls: number };
      const tracks = host.evaluation.parts.filter(part => part.animation?.length && !part.behavior).flatMap(part => part.animation ?? []);
      const duration = Math.max(0, ...tracks.map(track => track.keys[track.keys.length - 1].time));
      const next: Info = {
        duration, looping: tracks.some(track => track.looped), clock: behaviors.clock,
        playable: behaviors.parts.length > 0 || physics.movable > 0 || physics.ragdolls > 0, playing: behaviors.play,
        machine: host.evaluation.parts.some(part => part.state_machine),
      };
      setInfo(previous => JSON.stringify(previous) === JSON.stringify(next) ? previous : next);
    }, 250);
    return () => clearInterval(timer);
  }, []);
  return info;
}

/** The animation clock (play, pause, scrub, speed), Play mode for worlds that react to the visitor, and the controls of an animation state machine. */
export function Playback() {
  const info = useInfo();
  const [scrubbing, setScrubbing] = useState<number | null>(null);
  const [name, setName] = useState('');
  const [value, setValue] = useState('');
  const pressedAt = useRef(0);
  const [playMode, setPlayMode] = useState(false);
  useEffect(() => { if (performance.now() - pressedAt.current > 1500) setPlayMode(info.playing); }, [info.playing]);
  const runtime = rt();
  if (!runtime || (!info.duration && !info.playable && !info.machine)) return null;
  const shown = info.looping && info.duration ? info.clock.seconds % info.duration : Math.min(info.clock.seconds, info.duration);
  const seconds = scrubbing ?? shown;
  const go = () => {
    const n = name.trim(), v = value.trim();
    if (!n) return;
    if (v === '') runtime.animation_trigger('', n);
    else if (v === 'true' || v === 'false') runtime.animation_set_bool('', n, v === 'true');
    else if (Number.isFinite(Number(v))) runtime.animation_set_float('', n, Number(v));
  };
  return (
    <div className="playback glass" role="group" aria-label="Playback">
      {info.duration > 0 && <>
        <button className="btn icon" type="button" aria-label={info.clock.playing ? 'Pause animation' : 'Play animation'}
          onClick={() => runtime.viewer_animation(!info.clock.playing, -1)}>{info.clock.playing ? <Pause size={15} /> : <Play size={15} />}</button>
        <input className="range scrub" type="range" min={0} max={info.duration} step={0.01} value={seconds} aria-label="Animation time"
          onPointerDown={() => setScrubbing(shown)} onPointerUp={() => setScrubbing(null)}
          onChange={e => { setScrubbing(Number(e.target.value)); runtime.viewer_animation(false, Number(e.target.value)); }} />
        <output className="time">{seconds.toFixed(2)} / {info.duration.toFixed(2)} s</output>
        <select aria-label="Animation speed" value={String(info.clock.speed)} onChange={e => runtime.viewer_animation_speed(Number(e.target.value))}>
          {[0.25, 0.5, 1, 2].map(s => <option key={s} value={s}>{s}×</option>)}
        </select>
      </>}
      {info.machine && <>
        <label className="mini">speed <input className="range" type="range" min={0} max={4} step={0.05} defaultValue={0}
          onChange={e => runtime.animation_set_float('', 'speed', Number(e.target.value))} /></label>
        <button className="btn tonal" type="button" onClick={() => runtime.animation_trigger('', 'jump')}>jump</button>
        <button className="btn tonal" type="button" onClick={() => runtime.animation_set_bool('', 'grounded', false)}>in air</button>
        <button className="btn tonal" type="button" onClick={() => runtime.animation_set_bool('', 'grounded', true)}>on ground</button>
        <input className="txt small" placeholder="name" value={name} onChange={e => setName(e.target.value)} />
        <input className="txt small" placeholder="value (blank = trigger)" value={value} onChange={e => setValue(e.target.value)} onKeyDown={e => e.key === 'Enter' && go()} />
        <button className="btn tonal" type="button" onClick={go}>go</button>
      </>}
      {info.playable && (
        <button className={`btn ${playMode ? 'primary' : 'tonal'}`} type="button" aria-pressed={playMode}
          onClick={() => { pressedAt.current = performance.now(); setPlayMode(!playMode); runtime.viewer_play_mode(!playMode); }}>
          {playMode ? <><Square size={13} />Stop</> : <><Play size={13} />Play</>}
        </button>
      )}
    </div>
  );
}
