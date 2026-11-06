import type { ReactNode } from 'react';
import { Focus, RotateCcw } from 'lucide-react';
import { DEFAULT_EFFECTS, EFFECTS, GRADES, OVERLAYS } from './view';
import type { Lens, View } from './view';

function Switch({ label, hint, on, onChange }: { label: string; hint?: string; on: boolean; onChange: (on: boolean) => void }) {
  return (
    <label className="row switch-row">
      <span className="grow"><span className="lbl">{label}</span>{hint && <span className="hint">{hint}</span>}</span>
      <input className="switch" type="checkbox" role="switch" checked={on} onChange={e => onChange(e.target.checked)} />
    </label>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return <section className="vsec"><h3>{title}</h3>{children}</section>;
}

/** Quality, camera and look: the controls of the plain viewer's panel, as a glass popover. */
export function ViewPanel({ view, set, onFrame }: { view: View; set: <K extends keyof View>(key: K, value: View[K]) => void; onFrame: () => void }) {
  const tiers: View['quality'][] = ['low', 'quest', 'medium', 'high'];
  const lenses: [Lens, string][] = [['off', 'Flat'], ['fisheye', 'Fisheye'], ['headset', 'Headset']];
  return (
    <div className="viewpanel glass strong" role="dialog" aria-label="View and quality">
      <Section title="Quality">
        <div className="segmented wide" role="group" aria-label="Quality tier">
          {tiers.map(tier => <button key={tier} type="button" aria-pressed={view.quality === tier} onClick={() => set('quality', tier)}>{tier}</button>)}
        </div>
        <Switch label="Sun shadows" on={view.shadows} onChange={v => set('shadows', v)} />
        <label className="row"><span className="lbl grow">Shadow detail{view.shadows ? '' : ' (off)'}</span>
          <select value={view.shadowSize} disabled={!view.shadows} onChange={e => set('shadowSize', Number(e.target.value))}>
            <option value={0}>Auto</option><option value={512}>512</option><option value={1024}>1024</option><option value={2048}>2048</option><option value={4096}>4096</option>
          </select>
        </label>
        <Switch label="Anti-aliasing (SMAA)" hint="Smooths jagged edges" on={view.smaa} onChange={v => set('smaa', v)} />
        <Switch label="Ambient occlusion" hint="Contact shadows that seat surfaces in the scene" on={view.ssao} onChange={v => set('ssao', v)} />
        <Switch label="Specular anti-aliasing" hint="Calms shimmering highlights on fine relief" on={view.specularAa} onChange={v => set('specularAa', v)} />
        <Switch label="Bloom" on={view.bloom} onChange={v => set('bloom', v)} />
        <Switch label="Auto exposure" on={view.autoExposure} onChange={v => set('autoExposure', v)} />
      </Section>
      <Section title="Surface effects">
        {EFFECTS.map(([name, label, hint]) => (
          <Switch key={name} label={label} hint={hint} on={view.effects[name]} onChange={on => set('effects', { ...view.effects, [name]: on })} />
        ))}
        <div className="row actions">
          <button className="btn tonal" type="button" onClick={() => set('effects', DEFAULT_EFFECTS)}>Turn all on</button>
          <button className="btn tonal" type="button" onClick={() => set('effects', Object.fromEntries(EFFECTS.map(([n]) => [n, false])) as typeof view.effects)}>Plain (all off)</button>
        </div>
        <p className="keys">These only change how the viewer draws. The model keeps every effect it was given.</p>
      </Section>
      <Section title="Overlays">
        {OVERLAYS.map(([name, label, hint]) => (
          <Switch key={name} label={label} hint={hint} on={view.overlays[name]} onChange={on => set('overlays', { ...view.overlays, [name]: on })} />
        ))}
      </Section>
      <Section title="Look">
        <label className="row"><span className="lbl grow">Grade</span>
          <select value={view.grade} onChange={e => set('grade', e.target.value)}>{GRADES.map(g => <option key={g} value={g}>{g[0].toUpperCase() + g.slice(1)}</option>)}</select>
        </label>
        <label className="row"><span className="lbl">Exposure</span>
          <input className="range grow" type="range" min={0.25} max={4} step={0.05} value={view.exposure} onChange={e => set('exposure', Number(e.target.value))} />
          <span className="val">{view.exposure.toFixed(2)}×</span>
        </label>
      </Section>
      <Section title="Camera">
        <label className="row"><span className="lbl">Field of view</span>
          <input className="range grow" type="range" min={35} max={150} step={1} value={view.fov} onChange={e => set('fov', Number(e.target.value))} />
          <span className="val">{view.fov}°</span>
        </label>
        <div className="segmented wide" role="group" aria-label="Lens">
          {lenses.map(([lens, name]) => <button key={lens} type="button" aria-pressed={view.lens === lens} onClick={() => set('lens', lens)}>{name}</button>)}
        </div>
        <Switch label="Split-screen headset" hint="V" on={view.stereo} onChange={v => set('stereo', v)} />
        <Switch label="Drag to orbit" hint="O, or hold Alt and drag" on={view.orbit} onChange={v => set('orbit', v)} />
        <div className="row actions">
          <button className="btn tonal" type="button" onClick={onFrame}><Focus size={14} />Frame (F)</button>
          <button className="btn tonal" type="button" onClick={() => set('fov', 45)} title="Reset the field of view"><RotateCcw size={14} />Reset lens</button>
        </div>
      </Section>
      <p className="keys">Double-click the view to look around (Esc to let go). WASD move, Q and E down and up, Shift sprint, R reset the view, wheel zoom, H hide the panels.</p>
    </div>
  );
}
