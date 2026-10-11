// WebXR bridge. Everything crossing into Rust is a plain number, boolean or typed array: WebXR's
// interface types are not stable in web-sys, and copying a few small arrays per frame is cheap.

/** Floats per controller in `controllers()`: handedness, flags, grip(16), aim(16), buttons(8), axes(4). */
export const CONTROLLER_STRIDE = 46;

/** Whether an immersive VR session can be requested in this browser. */
export async function xrSupported() {
  if (!navigator.xr) return false;
  try {
    return await navigator.xr.isSessionSupported("immersive-vr");
  } catch {
    return false;
  }
}

export class XrBridge {
  constructor(session, layer, space) {
    this.session = session;
    this.layer = layer;
    this.space = space;
    this.frame = null;
    this.pose = null;
    this.ended = false;
    this.endPromise = new Promise((resolve) => session.addEventListener("end", () => { this.ended = true; resolve(); }));
  }

  /**
   * Requests a session and wires a WebGL layer for the canvas' context. `scale` multiplies the
   * runtime's recommended render resolution (the quality tier's setting).
   */
  static async request(canvas, scale) {
    const gl = canvas.getContext("webgl2");
    if (!gl) throw new Error("the canvas has no WebGL2 context");
    // Contexts made without xrCompatible must opt in before they can feed an XR layer.
    if (gl.makeXRCompatible) await gl.makeXRCompatible();
    const session = await navigator.xr.requestSession("immersive-vr", {
      requiredFeatures: ["local-floor"],
      optionalFeatures: ["hand-tracking"],
    });
    // No MSAA here: the engine resolves its own multisampled target. Depth is requested so a
    // compositor that wants it (for reprojection) can have it.
    const layer = new XRWebGLLayer(session, gl, {
      antialias: false, depth: true, stencil: false, alpha: false, framebufferScaleFactor: scale,
    });
    await session.updateRenderState({ baseLayer: layer, depthNear: 0.05 });
    const space = await session.requestReferenceSpace("local-floor");
    return new XrBridge(session, layer, space);
  }

  /** Calls `callback(timeMs)` once per XR frame, after the pose for that frame has been read. */
  start(callback) {
    const loop = (time, frame) => {
      if (this.ended) return;
      this.frame = frame;
      this.pose = frame.getViewerPose(this.space);
      callback(time);
      if (!this.ended) this.session.requestAnimationFrame(loop);
    };
    this.session.requestAnimationFrame(loop);
  }

  /** Calls `callback()` when the session ends, for any reason (user, system or `end()`). */
  onEnd(callback) { this.session.addEventListener("end", () => callback()); }

  isEnded() { return this.ended; }
  async end() { if (!this.ended) await this.session.end(); await this.endPromise; }

  /** True when the runtime tracked the head this frame (poses are valid). */
  poseValid() { return this.pose !== null; }
  viewCount() { return this.pose ? this.pose.views.length : 0; }
  viewWorldFromView(index) { return new Float32Array(this.pose.views[index].transform.matrix); }
  viewProjection(index) { return new Float32Array(this.pose.views[index].projectionMatrix); }

  /** `[x, y, width, height]` of a view in framebuffer pixels, origin bottom-left (GL convention). */
  viewViewport(index) {
    const v = this.layer.getViewport(this.pose.views[index]);
    return new Int32Array([v.x, v.y, v.width, v.height]);
  }

  framebufferSize() { return new Int32Array([this.layer.framebufferWidth, this.layer.framebufferHeight]); }

  /** The layer's framebuffer, or null when the runtime renders to the canvas' own framebuffer. */
  framebuffer() { return this.layer.framebuffer; }

  /** All tracked controllers, `CONTROLLER_STRIDE` floats each. */
  controllers() {
    const sources = [...this.session.inputSources].filter((s) => s.gripSpace || s.targetRaySpace);
    const out = new Float32Array(sources.length * CONTROLLER_STRIDE);
    sources.forEach((source, i) => {
      const base = i * CONTROLLER_STRIDE;
      out[base] = source.handedness === "left" ? 1 : source.handedness === "right" ? 2 : 0;
      const grip = source.gripSpace && this.frame.getPose(source.gripSpace, this.space);
      const aim = source.targetRaySpace && this.frame.getPose(source.targetRaySpace, this.space);
      out[base + 1] = (grip ? 1 : 0) + (aim ? 2 : 0);
      if (grip) out.set(grip.transform.matrix, base + 2);
      if (aim) out.set(aim.transform.matrix, base + 18);
      const pad = source.gamepad;
      if (pad) {
        pad.buttons.slice(0, 8).forEach((b, j) => { out[base + 34 + j] = b.value; });
        pad.axes.slice(0, 4).forEach((a, j) => { out[base + 42 + j] = a; });
      }
    });
    return out;
  }
}
