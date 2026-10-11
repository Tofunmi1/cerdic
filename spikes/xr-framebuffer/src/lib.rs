//! Spike: render into a `WebGLFramebuffer` that was created outside wgpu.
use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, WebGlFramebuffer};

#[wasm_bindgen]
pub struct Probe {
    device: wgpu::Device,
    queue: wgpu::Queue,
    _surface: wgpu::Surface<'static>,
}

#[wasm_bindgen]
impl Probe {
    /// Creates the wgpu instance on `canvas`, which makes wgpu acquire the canvas' WebGL2 context.
    pub async fn create(canvas: HtmlCanvasElement) -> Result<Probe, JsError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::GL,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|e| JsError::new(&format!("surface: {e}")))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(|e| JsError::new(&format!("adapter: {e}")))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .map_err(|e| JsError::new(&format!("device: {e}")))?;
        Ok(Probe { device, queue, _surface: surface })
    }

    /// Imports `framebuffer` as a wgpu texture and clears it to the given colour.
    pub fn clear_external(&self, framebuffer: WebGlFramebuffer, width: u32, height: u32, r: f64, g: f64, b: f64) -> Result<(), JsError> {
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let hal_desc = wgpu::hal::TextureDescriptor {
            label: Some("external_framebuffer"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUses::COLOR_TARGET,
            memory_flags: wgpu::hal::MemoryFlags::empty(),
            view_formats: vec![],
        };
        // SAFETY: the framebuffer belongs to the same GL context wgpu uses (same canvas) and its
        // colour attachment is an RGBA8 texture of this size.
        let texture = unsafe {
            let hal_device = self
                .device
                .as_hal::<wgpu::hal::api::Gles>()
                .ok_or_else(|| JsError::new("not a GLES device"))?;
            let hal_texture = hal_device.texture_from_web_framebuffer(framebuffer, &hal_desc, None);
            self.device.create_texture_from_hal::<wgpu::hal::api::Gles>(
                hal_texture,
                &wgpu::TextureDescriptor {
                    label: Some("external_framebuffer"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                },
                wgpu::TextureUses::UNINITIALIZED,
            )
        };
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear_external"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        self.queue.submit([encoder.finish()]);
        Ok(())
    }
}
