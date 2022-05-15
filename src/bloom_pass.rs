use std::mem::size_of;

use bytemuck::{bytes_of, Pod, Zeroable};

use crate::{entity::Scene, frame_buffers::FrameBuffers};

#[derive(Debug, Copy, Clone, Default, Pod, Zeroable)]
#[repr(C)]
struct BloomUniforms {
    intensity: f32,
    threshold: f32,
}

impl BloomUniforms {
    fn new(scene: &Scene) -> Self {
        Self {
            intensity: scene.bloom_effect.intensity,
            threshold: scene.bloom_effect.threshold,
        }
    }
}

pub struct BloomRenderer {
    bilinear_sampler: wgpu::Sampler,
    bright_uniform_buffer: wgpu::Buffer,
    bright_bind_group: wgpu::BindGroup,
    bright_bind_group_layout: wgpu::BindGroupLayout,
    bright_render_pipeline: wgpu::RenderPipeline,
    blur_bind_group_layout: wgpu::BindGroupLayout,
    blur_bind_group: wgpu::BindGroup,
    blur_render_pipeline: wgpu::RenderPipeline,
}

impl BloomRenderer {
    pub const STAGING_BUFFER_CHUNK_SIZE: wgpu::BufferAddress = size_of::<BloomUniforms>() as _;

    pub fn new(device: &wgpu::Device, frame_buffers: &FrameBuffers) -> Self {
        let bilinear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Bloom Bilinear Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let vertex_shader_module =
            device.create_shader_module(&wgpu::include_wgsl!("fullscreen_vs.wgsl"));

        let bright_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Bloom Uniform Buffer"),
            size: size_of::<BloomUniforms>() as _,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bright_bind_group_layout =
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
                                size_of::<BloomUniforms>() as _
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

        let bright_bind_group = Self::create_bright_bind_group(
            &device,
            &bright_bind_group_layout,
            &bright_uniform_buffer,
            &frame_buffers.color_texture_view,
            &bilinear_sampler,
        );

        let bright_render_pipeline = {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&bright_bind_group_layout],
                push_constant_ranges: &[],
            });

            let fragment_shader_module =
                device.create_shader_module(&wgpu::include_wgsl!("bloom_fs_bright.wgsl"));

            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vertex_shader_module,
                    entry_point: "main",
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &fragment_shader_module,
                    entry_point: "main",
                    targets: &[FrameBuffers::BLOOM_FORMAT.into()],
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

        let blur_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let blur_bind_group = Self::create_blur_bind_group(
            device,
            &blur_bind_group_layout,
            &frame_buffers.bright_texture_view,
            &bilinear_sampler,
        );

        let blur_render_pipeline = {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&blur_bind_group_layout],
                push_constant_ranges: &[],
            });

            let fragment_shader_module =
                device.create_shader_module(&wgpu::include_wgsl!("bloom_fs_blur.wgsl"));

            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vertex_shader_module,
                    entry_point: "main",
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &fragment_shader_module,
                    entry_point: "main",
                    targets: &[FrameBuffers::BLOOM_FORMAT.into()],
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

        Self {
            bilinear_sampler,
            bright_uniform_buffer,
            bright_bind_group_layout,
            bright_bind_group,
            bright_render_pipeline,
            blur_bind_group_layout,
            blur_bind_group,
            blur_render_pipeline,
        }
    }

    fn create_bright_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        uniform_buffer: &wgpu::Buffer,
        texture_view: &wgpu::TextureView,
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
                    resource: wgpu::BindingResource::TextureView(texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    fn create_blur_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        texture_view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    pub fn recreate_bind_group(&mut self, device: &wgpu::Device, frame_buffers: &FrameBuffers) {
        self.bright_bind_group = Self::create_bright_bind_group(
            device,
            &self.bright_bind_group_layout,
            &self.bright_uniform_buffer,
            &frame_buffers.color_texture_view,
            &self.bilinear_sampler,
        );
        self.blur_bind_group = Self::create_blur_bind_group(
            device,
            &self.blur_bind_group_layout,
            &frame_buffers.bright_texture_view,
            &self.bilinear_sampler,
        );
    }

    pub fn update(
        &self,
        device: &wgpu::Device,
        staging_belt: &mut wgpu::util::StagingBelt,
        encoder: &mut wgpu::CommandEncoder,
        scene: &Scene,
    ) {
        let bloom_uniforms = BloomUniforms::new(&scene);
        staging_belt
            .write_buffer(
                encoder,
                &self.bright_uniform_buffer,
                0,
                wgpu::BufferSize::new(size_of::<BloomUniforms>() as _).unwrap(),
                device,
            )
            .copy_from_slice(bytes_of(&bloom_uniforms));
    }

    pub fn draw(&self, encoder: &mut wgpu::CommandEncoder, frame_buffers: &FrameBuffers) {
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Bloom Bright Render Pass"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &frame_buffers.bright_texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });
            rpass.set_bind_group(0, &self.bright_bind_group, &[]);
            rpass.set_pipeline(&self.bright_render_pipeline);
            rpass.draw(0..3, 0..1);
        }
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Bloom Blur Render Pass"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &frame_buffers.bloom_texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });
            rpass.set_bind_group(0, &self.blur_bind_group, &[]);
            rpass.set_pipeline(&self.blur_render_pipeline);
            rpass.draw(0..3, 0..1);
        }
    }
}
