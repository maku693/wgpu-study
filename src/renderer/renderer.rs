use anyhow::{Context, Ok, Result};

use crate::{
    entity::Scene,
    window::{Size, Window},
};

use super::{
    particle::{ParticleRenderer, ParticleRendererBuilder},
    postprocessing::{
        AddRenderPass, BlurDownsampleRenderPass, BlurRenderPass, BlurUpsampleRenderPass,
        BrightPassRenderPass, ComposeRenderPass, CopyRenderPass,
    },
};

const HDR_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const DEPTH_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

pub struct Renderer {
    surface: wgpu::Surface,
    surface_format: wgpu::TextureFormat,
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

        Self::configure_surface(&surface, &device, surface_format, width, height);

        let render_targets = RenderTargets::new(&device, width, height);

        let particle_renderer = ParticleRendererBuilder::new(scene)
            .color_target_format(HDR_TEXTURE_FORMAT)
            .depth_format(DEPTH_TEXTURE_FORMAT)
            .build(&device);

        let bright_pass_render_pass = BrightPassRenderPass::new(
            &device,
            &render_targets.color.texture_view,
            HDR_TEXTURE_FORMAT,
        );

        let bloom_blur_downsample_render_passes = {
            let all_blur_texture_views_but_last = render_targets
                .bloom_blur_downsample
                .iter()
                .take(render_targets.bloom_blur_downsample.len() - 1);
            let src_texture_views =
                std::iter::once(&render_targets.bright_pass).chain(all_blur_texture_views_but_last);

            src_texture_views
                .map(|src_texture_view| {
                    BlurDownsampleRenderPass::new(
                        &device,
                        &src_texture_view.texture_view,
                        HDR_TEXTURE_FORMAT,
                        src_texture_view.width / 2,
                        src_texture_view.height / 2,
                    )
                })
                .collect::<Vec<_>>()
        };

        let bloom_blur_upsample_render_passes = {
            let last_blur_downsample_texture_view =
                render_targets.bloom_blur_downsample.last().into_iter();
            let all_blur_downsample_texture_views_but_last = render_targets
                .bloom_blur_upsample
                .iter()
                .take(render_targets.bloom_blur_upsample.len() - 1);
            let src_texture_views =
                last_blur_downsample_texture_view.chain(all_blur_downsample_texture_views_but_last);
            src_texture_views
                .map(|src_texture_view| {
                    BlurUpsampleRenderPass::new(
                        &device,
                        &src_texture_view.texture_view,
                        HDR_TEXTURE_FORMAT,
                        src_texture_view.width * 2,
                        src_texture_view.height * 2,
                    )
                })
                .collect::<Vec<_>>()
        };

        let compose_render_pass = ComposeRenderPass::new(
            &device,
            &render_targets.color.texture_view,
            &render_targets
                .bloom_blur_upsample
                .last()
                .unwrap()
                .texture_view,
            surface_format,
        );

        Ok(Self {
            surface,
            surface_format,
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
        )
    }

    pub fn resize(&mut self, size: Size) {
        let Size { width, height } = size;
        Self::configure_surface(
            &self.surface,
            &self.device,
            self.surface_format,
            width,
            height,
        );
        self.render_targets = RenderTargets::new(&self.device, width, height);
        self.bright_pass_render_pass = BrightPassRenderPass::new(
            &self.device,
            &self.render_targets.color.texture_view,
            HDR_TEXTURE_FORMAT,
        );
        self.compose_render_pass = ComposeRenderPass::new(
            &self.device,
            &self.render_targets.color.texture_view,
            &self
                .render_targets
                .bloom_blur_upsample
                .last()
                .unwrap()
                .texture_view,
            self.surface_format,
        );
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
    texture_view: wgpu::TextureView,
    width: u32,
    height: u32,
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
        let color = Self::create_render_target_texture_view(
            device,
            "Color Texture",
            width,
            height,
            HDR_TEXTURE_FORMAT,
        );
        let depth = Self::create_render_target_texture_view(
            device,
            "Depth Texture",
            width,
            height,
            DEPTH_TEXTURE_FORMAT,
        );
        let bright_pass = Self::create_render_target_texture_view(
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
                Self::create_render_target_texture_view(
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
                Self::create_render_target_texture_view(
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

    fn create_render_target_texture_view(
        device: &wgpu::Device,
        label: &str,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> RenderTarget {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
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
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        RenderTarget {
            texture_view,
            width,
            height,
        }
    }
}
