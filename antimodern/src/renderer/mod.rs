use anyhow::{Context, Ok, Result};

pub mod billboard;
pub mod cube;
pub mod particles;

pub struct Renderer {
    surface: wgpu::Surface,
    surface_format: wgpu::TextureFormat,
    device: wgpu::Device,
    queue: wgpu::Queue,
    depth_texture_view: wgpu::TextureView,
}

impl Renderer {
    const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub async fn new(instance: &wgpu::Instance, window: &winit::window::Window) -> Result<Self> {
        let surface = unsafe { instance.create_surface(window) };

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("No adapter found")?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .context("No device found")?;

        let winit::dpi::PhysicalSize { width, height } = window.inner_size();

        let surface_format = surface
            .get_preferred_format(&adapter)
            .context("There is no preferred format")?;
        Self::configure_surface(&surface, &device, surface_format, width, height);

        let depth_texture_view =
            Self::create_depth_texture_view(&device, Self::DEPTH_FORMAT, width, height);

        Ok(Self {
            surface,
            surface_format,
            device,
            queue,
            depth_texture_view,
        })
    }

    fn configure_surface(
        surface: &wgpu::Surface,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) {
        surface.configure(
            device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width,
                height,
                present_mode: wgpu::PresentMode::Fifo,
            },
        );
    }

    fn create_depth_texture_view(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> wgpu::TextureView {
        device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Depth texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            })
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("Depth texture view"),
                format: Some(Self::DEPTH_FORMAT),
                dimension: Some(wgpu::TextureViewDimension::D2),
                aspect: wgpu::TextureAspect::DepthOnly,
                base_mip_level: 0,
                mip_level_count: None,
                base_array_layer: 0,
                array_layer_count: None,
            })
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.surface_format
    }

    pub fn depth_texture_format(&self) -> wgpu::TextureFormat {
        Self::DEPTH_FORMAT
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn render(&self, pipeline: &impl Pipeline) {
        let frame_buffer = self
            .surface
            .get_current_texture()
            .expect("Failed to get next surface texture");

        let frame_buffer_view = frame_buffer.texture.create_view(&Default::default());

        let mut encoder = self.device.create_command_encoder(&Default::default());

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &frame_buffer_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                }],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });

            pipeline.render(&mut render_pass);
        }

        self.queue.submit(Some(encoder.finish()));

        frame_buffer.present();
    }

    pub fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        let Self {
            surface,
            device,
            surface_format,
            depth_texture_view,
            ..
        } = self;
        *depth_texture_view =
            Self::create_depth_texture_view(device, Self::DEPTH_FORMAT, size.width, size.height);
        Self::configure_surface(surface, device, *surface_format, size.width, size.height);
    }
}

pub trait Pipeline {
    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>);
}
