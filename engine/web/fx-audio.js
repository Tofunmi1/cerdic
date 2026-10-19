// Gesture-gated procedural audio for the built-in FX scene.
export function attachFxAudio({ api, params, button }) {
  let context = null;
  let enabled = false;
  let nextFountain = 0;
  const sources = [];
  const decodeSamples = (name, seed, intensity) => api.audio_sfx(name, seed, intensity);
  const syncListener = () => {
    if (!context || !api.viewer_pose) return;
    const raw = api.viewer_pose();
    if (!raw) return;
    const { position: p, rotation: q } = JSON.parse(raw);
    const [x, y, z, w] = q;
    const forward = [-(2 * (x * z + w * y)), -(2 * (y * z - w * x)), -(1 - 2 * (x * x + y * y))];
    const up = [2 * (x * y - w * z), 1 - 2 * (x * x + z * z), 2 * (y * z + w * x)];
    const listener = context.listener;
    listener.positionX.value = p[0]; listener.positionY.value = p[1]; listener.positionZ.value = p[2];
    listener.forwardX.value = forward[0]; listener.forwardY.value = forward[1]; listener.forwardZ.value = forward[2];
    listener.upX.value = up[0]; listener.upY.value = up[1]; listener.upZ.value = up[2];
  };
  const play = (name, seed, intensity, position, loop = false) => {
    if (!context || !enabled) return;
    syncListener();
    const samples = decodeSamples(name, seed, intensity);
    if (!samples?.length) return;
    const buffer = context.createBuffer(1, samples.length, 48000);
    buffer.copyToChannel(samples, 0);
    const source = context.createBufferSource();
    source.buffer = buffer;
    source.loop = loop;
    const gain = context.createGain();
    gain.gain.value = name.endsWith("loop") ? 0.18 : 0.65;
    const panner = context.createPanner();
    panner.panningModel = "HRTF";
    panner.positionX.value = position[0]; panner.positionY.value = position[1]; panner.positionZ.value = position[2];
    source.connect(gain).connect(panner).connect(context.destination);
    source.start();
    if (!loop) source.onended = () => { source.disconnect(); gain.disconnect(); panner.disconnect(); };
    else sources.push(source);
  };
  const startBed = () => {
    play("windloop", 7101, 0.28, [-3.2, 0.8, 0], true);
    play("rainloop", 7102, 0.24, [3.6, 2.0, 0], true);
  };
  button.addEventListener("click", async () => {
    enabled = !enabled;
    button.textContent = enabled ? "Sound on" : "Sound off";
    if (enabled) {
      context ??= new AudioContext();
      await context.resume();
      startBed();
    } else {
      for (const source of sources.splice(0)) { try { source.stop(); } catch {} }
    }
  });
  window.addEventListener("keydown", async (event) => {
    if (params.get("scene") !== "fx") return;
    if (!context) { context = new AudioContext(); }
    if (context.state === "suspended") await context.resume();
    if (!enabled) return;
    if (event.code === "KeyE") play("explosion", 7120, 0.9, [2, 0.3, -3.2]);
    const now = performance.now();
    if (now >= nextFountain) {
      nextFountain = now + 3200;
      play("splash", Math.floor(now), 0.45, [0, 0.3, 0]);
      play("whoosh", Math.floor(now) + 1, 0.24, [0, 0.3, 0]);
    }
  });
  return () => { enabled = false; for (const source of sources) { try { source.stop(); } catch {} } };
}
