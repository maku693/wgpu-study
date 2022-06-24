use anyhow::{Context, Ok, Result};

use crate::{
    entity::Scene,
    renderer::{
        particle::{ParticleRenderer, ParticleRendererBuilder},
        postprocessing::{BlurRenderer, BrightPassRenderer, CopyRenderer},
        render_target::RenderTarget,
    },
    window::{Size, Window},
};

use super::render_target::{DEPTH_TEXTURE_FORMAT, HDR_TEXTURE_FORMAT};

pub struct Renderer {
    surface: wgpu::Surface,
    surface_format: wgpu::TextureFormat,
    device: wgpu::Device,
    queue: wgpu::Queue,
    render_targets: RenderTargets,
    particle_renderer: ParticleRenderer,
    bright_pass_renderer: BrightPassRenderer,
    blur_renderer: BlurRenderer,
    blur_copy_renderer: CopyRenderer,
    // compose_renderer: CopyRenderer,
}

impl Renderer {
    pub async fn new(window: &impl Window, scene: &Scene) -> Result<Self> {
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
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await?;

        let size = window.size();

        Self::configure_surface(&surface, &device, surface_format, size);

        let render_targets = RenderTargets::new(&device, size);

        let particle_renderer = ParticleRendererBuilder::new(scene)
            .color_target_format(render_targets.color_target.format)
            .depth_format(render_targets.depth.format)
            .build(&device);

        let bright_pass_renderer = BrightPassRenderer::new(
            &device,
            size,
            render_targets.color_target.format,
            render_targets.bright_pass.format,
        );

        let blur_renderer = BlurRenderer::new(
            &device,
            &render_targets.color_target.texture_view,
            render_targets.blur[0].format,
        );

        let blur_copy_renderer = CopyRenderer::new(&device, render_targets.blur[0].format);

        // let compose_renderer =

        Ok(Self {
            surface,
            surface_format,
            device,
            queue,
            render_targets,
            particle_renderer,
            bright_pass_renderer,
            blur_renderer,
            blur_copy_renderer,
            // compose_renderer,
        })
    }

    fn configure_surface(
        surface: &wgpu::Surface,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        size: Size,
    ) {
        surface.configure(
            device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width: size.width,
                height: size.height,
                present_mode: wgpu::PresentMode::Fifo,
            },
        )
    }

    pub fn resize(&mut self, size: Size) {
        Self::configure_surface(&self.surface, &self.device, self.surface_format, size);
        self.render_targets = RenderTargets::new(&self.device, size);
        self.bright_pass_renderer = BrightPassRenderer::new(
            &self.device,
            size,
            self.render_targets.color_target.format,
            self.render_targets.bright_pass.format,
        )
    }

    pub fn render(&mut self, scene: &Scene) {
        self.particle_renderer.update(&self.queue, scene);
        self.bright_pass_renderer
            .update(&self.device, &self.queue, scene);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Base Passes Command Encoder"),
            });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Particle Render Pass"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &self.render_targets.color_target.texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: true,
                    },
                }],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.render_targets.depth.texture_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: false,
                    }),
                    stencil_ops: None,
                }),
            });
            self.particle_renderer.draw(&mut rpass);
        }

        self.queue.submit(Some(encoder.finish()));

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Post Processing Command Encoder"),
            });

        self.bright_pass_renderer.render(
            &mut encoder,
            &self.render_targets.color_target.texture,
            &self.render_targets.bright_pass.texture_view,
        );

        self.queue.submit(Some(encoder.finish()));

        let surface_texture = self
            .surface
            .get_current_texture()
            .expect("Failed to get next surface texture");

        let surface_texture_view = surface_texture.texture.create_view(&Default::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Composite Command Encoder"),
            });
        // {
        //     let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        //         label: Some("Copy Color Target Render Pass"),
        //         color_attachments: &[wgpu::RenderPassColorAttachment {
        //             view: &surface_texture_view,
        //             resolve_target: None,
        //             ops: wgpu::Operations {
        //                 load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
        //                 store: true,
        //             },
        //         }],
        //         depth_stencil_attachment: None,
        //     });
        //     self.composite_sample_texture_renderer
        //         .update_src_texture_view(
        //             &self.device,
        //             &self.render_targets.color_target.texture_view,
        //         );
        //     self.composite_sample_texture_renderer.draw(&mut rpass);
        // }

        // {
        //     let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        //         label: Some("Composite Post Process Effect Render Pass"),
        //         color_attachments: &[wgpu::RenderPassColorAttachment {
        //             view: &surface_texture_view,
        //             resolve_target: None,
        //             ops: wgpu::Operations {
        //                 load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
        //                 store: true,
        //             },
        //         }],
        //         depth_stencil_attachment: None,
        //     });
        //     self.compose_renderer
        //         .use_src_texture_view(&self.device, &self.render_targets.bright_pass.texture_view);
        //     self.compose_renderer.draw(&mut rpass);
        // }

        self.queue.submit(Some(encoder.finish()));

        surface_texture.present();
    }
}

struct RenderTargets {
    color_target: RenderTarget,
    depth: RenderTarget,
    bright_pass: RenderTarget,
    blur: Vec<RenderTarget>,
}

impl RenderTargets {
    fn new(device: &wgpu::Device, size: Size) -> Self {
        let color_target =
            RenderTarget::new(device, "Color Target Texture", HDR_TEXTURE_FORMAT, size);

        let depth = RenderTarget::new(device, "Depth Texture", DEPTH_TEXTURE_FORMAT, size);

        let bright_pass = RenderTarget::new(
            device,
            "Bright Pass Texture",
            HDR_TEXTURE_FORMAT,
            Size {
                width: size.width / 4,
                height: size.height / 4,
            },
        );

        let blur = (0..8)
            .map(|i| {
                let divisor = 4 * 2_u32.pow(i); // 4, 8, 16, 32, ..., 512
                RenderTarget::new(
                    device,
                    format!("Blur Texture {}", i).as_str(),
                    HDR_TEXTURE_FORMAT,
                    Size {
                        width: size.width / divisor,
                        height: size.height / divisor,
                    },
                )
            })
            .collect::<Vec<_>>();

        Self {
            color_target,
            depth,
            bright_pass,
            blur,
        }
    }
}
