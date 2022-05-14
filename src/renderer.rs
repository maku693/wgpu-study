use std::{future::Future, mem::size_of};

use anyhow::{Context, Ok, Result};
use bytemuck::{bytes_of, Pod, Zeroable};

use winit::{dpi::PhysicalSize, window::Window};

use crate::{
    entity::Scene, frame_buffers::FrameBuffers, particle_pass::ParticleRenderer, surface::Surface,
};

#[derive(Debug, Copy, Clone, Default, Pod, Zeroable)]
#[repr(C)]
struct CompositeUniforms {
    exposure: f32,
}

impl CompositeUniforms {
    fn new(scene: &Scene) -> Self {
        Self {
            exposure: scene.camera.exposure,
        }
    }
}

pub struct Renderer {
    surface: Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    frame_buffers: FrameBuffers,
    particle_renderer: ParticleRenderer,
    staging_belt: wgpu::util::StagingBelt,
    composite_uniform_buffer: wgpu::Buffer,
    composite_bind_group_layout: wgpu::BindGroupLayout,
    composite_bind_group: wgpu::BindGroup,
    composite_render_pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
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
                + (size_of::<CompositeUniforms>() as wgpu::BufferAddress),
        );

        let PhysicalSize { width, height } = window.inner_size();

        let surface = Surface::new(surface, surface_format);
        surface.configure(&device, width, height);

        let frame_buffers = FrameBuffers::new(&device, width, height);

        let particle_renderer = ParticleRenderer::new(&device, scene);

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let composite_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Composite pass uniform buffer"),
            size: size_of::<CompositeUniforms>() as _,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let composite_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(
                                size_of::<CompositeUniforms>() as _
                            ),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let composite_bind_group = Self::create_composite_bind_group(
            &device,
            &composite_bind_group_layout,
            &composite_uniform_buffer,
            &frame_buffers.color_texture_view,
            &sampler,
        );

        let composite_render_pipeline = {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&composite_bind_group_layout],
                push_constant_ranges: &[],
            });

            let shader_module = device.create_shader_module(&wgpu::include_wgsl!("composite.wgsl"));

            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader_module,
                    entry_point: "vs_main",
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader_module,
                    entry_point: "fs_main",
                    targets: &[surface_format.into()],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            })
        };

        Ok(Self {
            surface,
            device,
            queue,
            staging_belt,
            frame_buffers,
            particle_renderer,
            composite_uniform_buffer,
            composite_bind_group_layout,
            composite_bind_group,
            composite_render_pipeline,
            sampler,
        })
    }

    fn create_composite_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        uniform_buffer: &wgpu::Buffer,
        color_texture_view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(color_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.surface.configure(&self.device, width, height);

        self.frame_buffers.resize(&self.device, width, height);

        self.composite_bind_group = Self::create_composite_bind_group(
            &self.device,
            &self.composite_bind_group_layout,
            &self.composite_uniform_buffer,
            &self.frame_buffers.color_texture_view,
            &self.sampler,
        );
    }

    pub fn render(&mut self, scene: &Scene) -> impl Future<Output = ()> {
        let composite_uniforms = CompositeUniforms::new(&scene);

        let surface_texture = self
            .surface
            .wgpu_surface
            .get_current_texture()
            .expect("Failed to get next surface texture");

        let surface_texture_view = surface_texture.texture.create_view(&Default::default());

        let mut encoder = self.device.create_command_encoder(&Default::default());

        self.particle_renderer
            .update(&self.device, &mut self.staging_belt, &mut encoder, scene);

        self.staging_belt
            .write_buffer(
                &mut encoder,
                &self.composite_uniform_buffer,
                0,
                wgpu::BufferSize::new(size_of::<CompositeUniforms>() as _).unwrap(),
                &self.device,
            )
            .copy_from_slice(bytes_of(&composite_uniforms));

        self.staging_belt.finish();

        self.particle_renderer
            .draw(&self.frame_buffers, &mut encoder);

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &surface_texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });
            rpass.set_bind_group(0, &self.composite_bind_group, &[]);
            rpass.set_pipeline(&self.composite_render_pipeline);
            rpass.draw(0..3, 0..1);
        }

        self.queue.submit(Some(encoder.finish()));

        surface_texture.present();

        self.staging_belt.recall()
    }
}
