# ADR 0049: Automatic exposure on the GPU, through the view bind group

- Status: accepted
- Date: 2026-10-19

## Context

Exposure was a manual number (`Grading::exposure`, a viewer slider) multiplied into the scene inside the forward shader. A scene lit very differently from the one it was tuned for came out dark or blown out, and a person or a model
has no way to know the right number in advance. Eye adaptation is the usual answer. The browser backend makes the obvious implementation slow: reading the picture back to the CPU stalls the frame, and a depth-style texture read is
not available (see ADR 0048). The sky is also display-referred and never saw exposure at all, so a scene-only multiplier would have changed the scene against a fixed sky.

## Decision

1. **A one-pixel pass measures, a 1 by 1 float texture holds the answer** (`exposure.rs`, `shaders/exposure.wgsl`). After the forward pass, a pass over one pixel takes the geometric mean of the picture's brightness over a 24 by 24 grid
   (brightness capped at 1 and floored, so clipped highlights and black pixels do not dominate), compares it with 18% grey, and writes the next exposure into one of two `R32Float` textures that swap each frame. Nothing is read back to
   the CPU.
2. **The exposure texture is part of the shared view bind group** (binding 1 of the view layout). Every pass that binds the view (forward, sky, the depth pass, the exposure pass itself) can read it. The forward shader multiplies the
   scene by it after the manual exposure, so the slider becomes a bias on top; the sky multiplies by it too, so the whole picture responds as through a camera. While auto exposure is off the texture holds 1 and nothing changes. The sun's
   shadow pass builds its own bind group, so it keeps a uniform-only layout.
3. **A feedback controller, because tonemapping is inside the forward shader.** The measurement is of the displayed picture, not scene radiance. Each frame moves the exposure by `error * 0.6 * adaptation` stops (error limited to 3 stops,
   total range plus or minus 6 stops, a factor of 64 each way). `adaptation = 1 - exp(-2 * dt)` with `dt` capped at half a second, so it takes about a third of a second to cover half the error at any frame rate. The host publishes
   `dt` each frame as `ve_render::FrameDelta`. The maths is mirrored in Rust (`next_exposure`, test-only) and tested; a test keeps the shader's constants equal to the Rust ones.
4. **Off by default**, a `RenderQuality` flag like the other post effects (`?autoexposure=on`, the viewer checkbox, `set_auto_exposure`). Enabling it renders offscreen (the picture must be sampled), as AO and bloom do. The flag survives a
   tier change.

## Consequences

- A browser test (`web/tests/g11-auto-exposure.mjs`) uses the manual slider as the disturbance: off, a quarter and four times the exposure give 61 and 224 (mean sRGB); on, both settle at 110 (18% grey is 118); off again restores
  224. A scene lit very differently now ends at a similar brightness without anyone tuning it.
- The meter weights the whole picture equally, so a large bright sky pulls the exposure down and darkens the scene (as a camera does). Centre-weighted or spot metering, and a separate adaptation speed for getting brighter or darker, are
  not built.
- It adds one pass over one pixel (576 texture reads) per frame, and one extra texture binding in the view group; the cost on a real GPU is negligible but not measured.
- With several cameras the last one measured wins; stereo meters the first eye's viewport only, which both eyes then share.
- Bloom, AO and the grade run on the picture after exposure; bloom's threshold therefore moves with exposure, which is what one wants.
