# Patches to vendored code

One patch = one purpose, with the upstream reference and why it cannot live outside the fork.
Keep each change as a separate commit so it can be rebased onto a newer upstream release.

| # | Crate | Purpose | Upstream status |
|---|---|---|---|
| 1 | wgpu-hal 30.0.1 (`src/gles/device.rs`) | Adds `Device::texture_from_web_framebuffer`, a public constructor for `TextureInner::ExternalFramebuffer`. Upstream already renders to that variant (see `command.rs`, `queue.rs`) but nothing can create one on the web, and `texture_from_raw` is native-only. We need it to render into a WebXR layer's framebuffer. | Not proposed upstream yet. Worth upstreaming once proven on a headset. |
