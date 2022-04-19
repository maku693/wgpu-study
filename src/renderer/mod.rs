use anyhow::{Context, Ok, Result};

pub struct Renderer {
    surface: wgpu::Surface,
    surface_configuration: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl Renderer {
    pub async fn new(
        instance: &wgpu::Instance,
        window: &winit::window::Window,
    ) -> Result<Renderer> {
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

        let surface_configuration = {
            let surface_format = surface
                .get_preferred_format(&adapter)
                .context("There is no preferred format")?;

            let winit::dpi::PhysicalSize { width, height } = window.inner_size();

            wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width,
                height,
                present_mode: wgpu::PresentMode::Fifo,
            }
        };
        surface.configure(&device, &surface_configuration);

        Ok(Renderer {
            surface,
            surface_configuration,
            device,
            queue,
        })
    }

    pub fn surface(&self) -> &wgpu::Surface {
        &self.surface
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.surface_configuration.format
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn render<P>(&self, pipeline: &P)
    where
        P: Pipeline,
    {
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
                depth_stencil_attachment: None,
            });
            pipeline.render(&mut render_pass);
        }

        self.queue.submit(Some(encoder.finish()));

        frame_buffer.present();
    }

    pub fn resize_surface(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        self.surface_configuration.width = size.width;
        self.surface_configuration.height = size.height;
        self.surface
            .configure(&self.device, &self.surface_configuration)
    }
}

pub trait Pipeline {
    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>);
}
