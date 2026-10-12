import type { Runtime } from './bridge';

export function bindInput(canvas: HTMLCanvasElement, runtime: Runtime) {
  const abort = new AbortController();
  const options = { signal: abort.signal };
  const keys = new Set<string>();
  const motion = () => runtime.viewer_move(
    Number(keys.has('KeyD')) - Number(keys.has('KeyA')),
    Number(keys.has('KeyE')) - Number(keys.has('KeyQ')),
    Number(keys.has('KeyW')) - Number(keys.has('KeyS')), keys.has('ShiftLeft'));
  const release = () => {
    keys.clear(); motion(); runtime.viewer_set_locked(false);
    if (document.pointerLockElement === canvas) document.exitPointerLock();
  };
  canvas.addEventListener('pointerdown', () => canvas.focus(), options);
  canvas.addEventListener('dblclick', () => {
    canvas.requestPointerLock()?.catch(() => {});
  }, options);
  canvas.addEventListener('keydown', e => {
    if (['KeyW', 'KeyA', 'KeyS', 'KeyD', 'KeyQ', 'KeyE', 'ShiftLeft'].includes(e.code)) {
      e.preventDefault(); keys.add(e.code); motion();
    }
  }, options);
  canvas.addEventListener('keyup', e => { keys.delete(e.code); motion(); }, options);
  canvas.addEventListener('blur', release, options);
  document.addEventListener('pointerlockchange', () => {
    const locked = document.pointerLockElement === canvas;
    runtime.viewer_set_locked(locked);
    if (!locked) { keys.clear(); motion(); }
  }, options);
  document.addEventListener('mousemove', e => {
    if (document.pointerLockElement === canvas) runtime.viewer_look(e.movementX, e.movementY);
  }, options);
  window.addEventListener('blur', release, options);
  return () => { release(); abort.abort(); };
}
