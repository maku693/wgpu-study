use std::future::Future;

use anyhow::{Context, Ok, Result};
use winit::{dpi::PhysicalSize, window::Window};

use crate::{
    bloom_pass::BloomRenderer, composite_pass::CompositeRenderer, entity::Scene,
    frame_buffers::FrameBuffers, particle_pass::ParticleRenderer, surface::Surface,
};

pub struct Renderer {
    surface: Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    staging_belt: wgpu::util::StagingBelt,
    frame_buffers: FrameBuffers,
    particle_renderer: ParticleRenderer,
    bloom_renderer: BloomRenderer,
    composite_renderer: CompositeRenderer,
}

impl Renderer {
    pub async fn new(window: &Window, scene: &Scene) -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);
        let surface = unsafe { instance.create_surface(&window) };

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("No adapter found")?;

        let surface_format = surface
            .get_preferred_format(&adapter)
            .context("No preferred format found")?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                },
                None,
            )
            .await?;

        let staging_belt = wgpu::util::StagingBelt::new(
            ParticleRenderer::STAGING_BUFFER_CHUNK_SIZE
                + CompositeRenderer::STAGING_BUFFER_CHUNK_SIZE,
        );

        let PhysicalSize { width, height } = window.inner_size();

        let surface = Surface::new(surface, surface_format);
        surface.configure(&device, width, height);

        let frame_buffers = FrameBuffers::new(&device, width, height);

        let particle_renderer = ParticleRenderer::new(&device, scene);
        let bloom_renderer = BloomRenderer::new(&device, &frame_buffers);
        let composite_renderer = CompositeRenderer::new(&device, &frame_buffers, &surface);

        Ok(Self {
            surface,
            device,
            queue,
            staging_belt,
            frame_buffers,
            particle_renderer,
            bloom_renderer,
            composite_renderer,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.surface.configure(&self.device, width, height);

        self.frame_buffers.resize(&self.device, width, height);

        self.bloom_renderer
            .recreate_bind_group(&self.device, &self.frame_buffers);
        self.composite_renderer
            .recreate_bind_group(&self.device, &self.frame_buffers);
    }

    pub fn render(&mut self, scene: &Scene) -> impl Future<Output = ()> {
        let surface_texture = self
            .surface
            .wgpu_surface
            .get_current_texture()
            .expect("Failed to get next surface texture");

        let surface_texture_view = surface_texture.texture.create_view(&Default::default());

        let mut encoder = self.device.create_command_encoder(&Default::default());

        self.particle_renderer
            .update(&self.device, &mut self.staging_belt, &mut encoder, scene);
        self.bloom_renderer
            .update(&self.device, &mut self.staging_belt, &mut encoder, scene);
        self.composite_renderer
            .update(&self.device, &mut self.staging_belt, &mut encoder, scene);

        self.staging_belt.finish();

        self.particle_renderer
            .draw(&mut encoder, &self.frame_buffers);

        self.bloom_renderer.draw(&mut encoder, &self.frame_buffers);

        self.composite_renderer
            .draw(&mut encoder, &surface_texture_view);

        self.queue.submit(Some(encoder.finish()));

        surface_texture.present();

        self.staging_belt.recall()
    }
}
