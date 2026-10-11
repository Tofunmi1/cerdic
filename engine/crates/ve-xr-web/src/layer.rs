//! Connects the XR layer's framebuffer to the headset camera. Browser only.
//!
//! Real headsets hand out an opaque WebGL framebuffer that wgpu did not create. It is imported as a
//! wgpu texture (through the patched wgpu-hal, see `third_party/PATCHES.md`), registered as a manual
//! texture view and set as the headset camera's render target. The framebuffer is not an sRGB view,
//! so `ve-render` renders offscreen and its present pass encodes into it. Emulated runtimes have no
//! framebuffer (they draw to the canvas), in which case the camera keeps rendering to the window.
#![expect(
    unsafe_code,
    reason = "importing a framebuffer created outside wgpu needs wgpu-hal's unsafe constructor"
)]

use bevy_camera::{ManualTextureViewHandle, RenderTarget};
use bevy_ecs::query::With;
use bevy_ecs::system::{Local, Query, Res, ResMut};
use bevy_math::UVec2;
use bevy_render::renderer::RenderDevice;
use bevy_render::texture::{ManualTextureView, ManualTextureViews};
use web_sys::WebGlFramebuffer;

use crate::plugin::XrCamera;
use crate::session::layer_framebuffer;
use crate::state::{SessionState, session_state};

/// The handle the layer's framebuffer is registered under.
const LAYER_VIEW: ManualTextureViewHandle = ManualTextureViewHandle(0x5852);

/// The default `WebXR` layer colour format. The framebuffer is not an sRGB view.
const LAYER_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// How the headset camera is currently connected to the session.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Binding {
    /// No session is presenting.
    #[default]
    Unbound,
    /// The session draws to the page's canvas (emulators); the camera keeps its window target.
    Canvas,
    /// The camera renders into the layer's imported framebuffer.
    Layer,
}

/// Wraps `framebuffer` as a wgpu texture view of the given size.
fn import(
    device: &RenderDevice,
    framebuffer: WebGlFramebuffer,
    size: UVec2,
) -> Option<ManualTextureView> {
    let extent = wgpu::Extent3d {
        width: size.x,
        height: size.y,
        depth_or_array_layers: 1,
    };
    let hal_desc = wgpu::hal::TextureDescriptor {
        label: Some("ve_xr_layer"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: LAYER_FORMAT,
        usage: wgpu::TextureUses::COLOR_TARGET,
        memory_flags: wgpu::hal::MemoryFlags::empty(),
        view_formats: vec![],
    };
    let wgpu_device = device.wgpu_device();
    // SAFETY: the framebuffer is the session layer's, created on the same WebGL context wgpu renders
    // with (the canvas was made XR-compatible before the context was used), and its colour attachment
    // is an RGBA8 texture of the stated size. The view keeps the imported texture alive; the
    // framebuffer itself is owned by the session and outlives every frame that uses it.
    let texture = unsafe {
        let hal_device = wgpu_device.as_hal::<wgpu::hal::api::Gles>()?;
        let hal_texture = hal_device.texture_from_web_framebuffer(framebuffer, &hal_desc, None);
        wgpu_device.create_texture_from_hal::<wgpu::hal::api::Gles>(
            hal_texture,
            &wgpu::TextureDescriptor {
                label: Some("ve_xr_layer"),
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: LAYER_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            },
            wgpu::TextureUses::UNINITIALIZED,
        )
    };
    Some(ManualTextureView {
        texture_view: texture
            .create_view(&wgpu::TextureViewDescriptor::default())
            .into(),
        size,
        view_format: LAYER_FORMAT,
    })
}

/// Points the headset camera at the layer's framebuffer while a session is presenting, and back at
/// the window when it ends.
pub(crate) fn bind_layer_framebuffer(
    mut binding: Local<Binding>,
    device: Option<Res<RenderDevice>>,
    views: Option<ResMut<ManualTextureViews>>,
    mut cameras: Query<&mut RenderTarget, With<XrCamera>>,
) {
    let Some(mut views) = views else {
        return;
    };
    if session_state() != SessionState::Presenting {
        if *binding == Binding::Layer {
            views.remove(&LAYER_VIEW);
            for mut target in &mut cameras {
                *target = RenderTarget::default();
            }
        }
        *binding = Binding::Unbound;
        return;
    }
    if *binding != Binding::Unbound {
        return;
    }
    let Some(device) = device else {
        return;
    };
    let Some((framebuffer, width, height)) = layer_framebuffer() else {
        *binding = Binding::Canvas;
        return;
    };
    let Some(view) = import(&device, framebuffer, UVec2::new(width, height)) else {
        crate::state::set_last_error("the GPU backend cannot import the XR framebuffer".to_owned());
        *binding = Binding::Canvas;
        return;
    };
    views.insert(LAYER_VIEW, view);
    for mut target in &mut cameras {
        *target = RenderTarget::TextureView(LAYER_VIEW);
    }
    *binding = Binding::Layer;
}
