use anyhow::{Context, Ok, Result};

use crate::{
    entity::Scene,
    window::{Size, Window},
};

use super::{
    particle::ParticleRenderer,
    postprocessing::{
        AddRenderPass, BlurDownsampleRenderPass, BlurRenderPass, BlurUpsampleRenderPass,
        BrightPassRenderPass, ComposeRenderPass, CopyRenderPass,
    },
    wgpu_ext::{self, DeviceExt},
};

const HDR_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const DEPTH_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

pub struct Renderer {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    render_targets: RenderTargets,
    particle_renderer: ParticleRenderer,
    bright_pass_render_pass: BrightPassRenderPass,
    bloom_blur_downsample_render_passes: Vec<BlurDownsampleRenderPass>,
    bloom_blur_upsample_render_passes: Vec<BlurUpsampleRenderPass>,
    compose_render_pass: ComposeRenderPass,
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

        let Size { width, height } = window.size();

        surface.configure(
            &device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width,
                height,
                present_mode: wgpu::PresentMode::Fifo,
            },
        );

        let render_targets = RenderTargets::new(&device, width, height);

        let particle_renderer = ParticleRenderer::new(
            &device,
            render_targets.color.texture.format(),
            render_targets.depth.texture.format(),
            scene,
        );

        let bright_pass_render_pass = BrightPassRenderPass::new(
            &device,
            render_targets.color.texture.wgpu_texture(),
            render_targets.bright_pass.texture.format(),
        );

        let bloom_blur_downsample_render_passes = {
            let dst = &render_targets.bloom_blur_downsample;

            let src =
                std::iter::once(&render_targets.bright_pass).chain(dst.iter().take(dst.len() - 1));

            std::iter::zip(src, dst)
                .map(|(src, dst)| {
                    BlurDownsampleRenderPass::new(&device, &src.texture, &dst.texture)
                })
                .collect::<Vec<_>>()
        };

        let bloom_blur_upsample_render_passes = {
            let dst = &render_targets.bloom_blur_upsample;
            let src = render_targets
                .bloom_blur_downsample
                .last()
                .into_iter()
                .chain(dst.iter().take(dst.len() - 1));

            std::iter::zip(src, dst)
                .map(|(src, dst)| BlurUpsampleRenderPass::new(&device, &src.texture, &dst.texture))
                .collect::<Vec<_>>()
        };

        let compose_render_pass = ComposeRenderPass::new(
            &device,
            render_targets.color.texture.wgpu_texture(),
            render_targets
                .bloom_blur_upsample
                .last()
                .unwrap()
                .texture
                .wgpu_texture(),
            surface_format,
        );

        Ok(Self {
            surface,
            device,
            queue,
            render_targets,
            particle_renderer,
            bright_pass_render_pass,
            bloom_blur_downsample_render_passes,
            bloom_blur_upsample_render_passes,
            compose_render_pass,
        })
    }

    pub fn render(&mut self, scene: &Scene) {
        self.particle_renderer.update(&self.queue, scene);
        self.bright_pass_render_pass.update(&self.queue, scene);
        self.compose_render_pass.update(&self.queue, scene);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Particle Render Pass"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &self.render_targets.color.texture_view,
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

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Bright Pass Render Pass"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &self.render_targets.bright_pass.texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });
            self.bright_pass_render_pass.draw(&mut rpass);
        }

        for i in 0..self.render_targets.bloom_blur_downsample.len() {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(format!("Bloom Blur Downsample Render Pass {}", i).as_str()),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &self.render_targets.bloom_blur_downsample[i].texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });
            self.bloom_blur_downsample_render_passes[i].draw(&mut rpass);
        }

        for (i, render_target) in self.render_targets.bloom_blur_upsample.iter().enumerate() {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(format!("Bloom Blur Upsample Render Pass {}", i).as_str()),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &render_target.texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });
            self.bloom_blur_upsample_render_passes[i].draw(&mut rpass);
        }

        let surface_texture = self
            .surface
            .get_current_texture()
            .expect("Failed to get next surface texture");

        let surface_texture_view = surface_texture.texture.create_view(&Default::default());

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Compose Render Pass"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &surface_texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });
            self.compose_render_pass.draw(&mut rpass);
        }

        self.queue.submit(std::iter::once(encoder.finish()));

        surface_texture.present();
    }
}

struct RenderTarget {
    texture: wgpu_ext::Texture,
    texture_view: wgpu::TextureView,
}

impl RenderTarget {
    fn new(
        device: &wgpu::Device,
        label: &str,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> RenderTarget {
        let texture = device.create_texture_ext(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        });

        let texture_view = texture
            .wgpu_texture()
            .create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            texture,
            texture_view,
        }
    }
}

struct RenderTargets {
    color: RenderTarget,
    depth: RenderTarget,
    bright_pass: RenderTarget,
    bloom_blur_downsample: Vec<RenderTarget>,
    bloom_blur_upsample: Vec<RenderTarget>,
}

impl RenderTargets {
    fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let color = RenderTarget::new(device, "Color Texture", width, height, HDR_TEXTURE_FORMAT);
        let depth = RenderTarget::new(device, "Depth Texture", width, height, DEPTH_TEXTURE_FORMAT);
        let bright_pass = RenderTarget::new(
            device,
            "Bright Pass Texture",
            width,
            height,
            HDR_TEXTURE_FORMAT,
        );

        let base_divisor = 2;
        let num_levels = 4;
        let bloom_blur_downsample = (0..num_levels)
            .map(|i| {
                let divisor = base_divisor * 2u32.pow(1 + i); // 2, 4, 8, 16
                RenderTarget::new(
                    device,
                    format!("Bloom Blur Downsample Texture {}", i).as_str(),
                    width / divisor,
                    height / divisor,
                    HDR_TEXTURE_FORMAT,
                )
            })
            .collect::<Vec<_>>();
        let bloom_blur_upsample = (0..num_levels)
            .rev()
            .map(|i| {
                let divisor = base_divisor * 2u32.pow(i); // 8, 4, 2, 1
                RenderTarget::new(
                    device,
                    format!("Bloom Blur Upsample Texture {}", i).as_str(),
                    width / divisor,
                    height / divisor,
                    HDR_TEXTURE_FORMAT,
                )
            })
            .collect::<Vec<_>>();

        Self {
            color,
            depth,
            bright_pass,
            bloom_blur_downsample,
            bloom_blur_upsample,
        }
    }
}
