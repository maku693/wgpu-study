use std::mem::size_of;

use bytemuck::{bytes_of, Pod, Zeroable};

use crate::{entity::Scene, frame_buffers::FrameBuffers, samplers::Samplers};

pub struct BloomRenderer {
    bright_pass: BrightPass,
    blur_pass: BlurPass,
}

impl BloomRenderer {
    pub const STAGING_BUFFER_CHUNK_SIZE: wgpu::BufferAddress = size_of::<BrightUniforms>() as _;

    pub fn new(device: &wgpu::Device, frame_buffers: &FrameBuffers, samplers: &Samplers) -> Self {
        let bright_pass = BrightPass::new(device, frame_buffers, samplers);
        let blur_pass = BlurPass::new(device, frame_buffers, samplers);

        Self {
            bright_pass,
            blur_pass,
        }
    }

    pub fn recreate_bind_group(
        &mut self,
        device: &wgpu::Device,
        frame_buffers: &FrameBuffers,
        samplers: &Samplers,
    ) {
        self.bright_pass
            .recreate_bind_group(device, frame_buffers, samplers);
        self.blur_pass
            .recreate_bind_group(device, frame_buffers, samplers);
    }

    pub fn update(
        &self,
        device: &wgpu::Device,
        staging_belt: &mut wgpu::util::StagingBelt,
        encoder: &mut wgpu::CommandEncoder,
        scene: &Scene,
    ) {
        self.bright_pass
            .update(device, staging_belt, encoder, scene);
    }

    pub fn draw(&self, encoder: &mut wgpu::CommandEncoder, frame_buffers: &FrameBuffers) {
        self.bright_pass.draw(encoder, frame_buffers);
        self.blur_pass.draw(encoder, frame_buffers);
    }
}

#[derive(Debug, Copy, Clone, Default, Pod, Zeroable)]
#[repr(C)]
struct BrightUniforms {
    intensity: f32,
    threshold: f32,
}

impl BrightUniforms {
    fn new(scene: &Scene) -> Self {
        Self {
            intensity: scene.bloom_effect.intensity,
            threshold: scene.bloom_effect.threshold,
        }
    }
}

struct BrightPass {
    bright_uniform_buffer: wgpu::Buffer,
    bright_bind_group: wgpu::BindGroup,
    bright_bind_group_layout: wgpu::BindGroupLayout,
    bright_render_pipeline: wgpu::RenderPipeline,
}

impl BrightPass {
    pub fn new(device: &wgpu::Device, frame_buffers: &FrameBuffers, samplers: &Samplers) -> Self {
        let vertex_shader_module =
            device.create_shader_module(&wgpu::include_wgsl!("fullscreen_vs.wgsl"));

        let bright_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Bloom Uniform Buffer"),
            size: size_of::<BrightUniforms>() as _,
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
                                size_of::<BrightUniforms>() as _
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
            &samplers.bilinear,
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

        Self {
            bright_uniform_buffer,
            bright_bind_group_layout,
            bright_bind_group,
            bright_render_pipeline,
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

    pub fn recreate_bind_group(
        &mut self,
        device: &wgpu::Device,
        frame_buffers: &FrameBuffers,
        samplers: &Samplers,
    ) {
        self.bright_bind_group = Self::create_bright_bind_group(
            device,
            &self.bright_bind_group_layout,
            &self.bright_uniform_buffer,
            &frame_buffers.color_texture_view,
            &samplers.bilinear,
        );
    }

    pub fn update(
        &self,
        device: &wgpu::Device,
        staging_belt: &mut wgpu::util::StagingBelt,
        encoder: &mut wgpu::CommandEncoder,
        scene: &Scene,
    ) {
        let bloom_uniforms = BrightUniforms::new(&scene);
        staging_belt
            .write_buffer(
                encoder,
                &self.bright_uniform_buffer,
                0,
                wgpu::BufferSize::new(size_of::<BrightUniforms>() as _).unwrap(),
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
    }
}

struct DownScale {
    bind_group: wgpu::BindGroup,
    bind_group_layout: wgpu::BindGroupLayout,
    render_pipeline: wgpu::RenderPipeline,
}

impl DownScale {
    pub fn new(device: &wgpu::Device, frame_buffers: &FrameBuffers, samplers: &Samplers) -> Self {
        let vertex_shader_module =
            device.create_shader_module(&wgpu::include_wgsl!("fullscreen_vs.wgsl"));

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        let bind_group = Self::create_bind_group(
            &device,
            &bind_group_layout,
            &frame_buffers.color_texture_view,
            &samplers.bilinear,
        );

        let render_pipeline = {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

            let fragment_shader_module =
                device.create_shader_module(&wgpu::include_wgsl!("draw_texture_fs.wgsl"));

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
            bind_group_layout,
            bind_group,
            render_pipeline,
        }
    }

    fn create_bind_group(
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

    pub fn recreate_bind_group(
        &mut self,
        device: &wgpu::Device,
        frame_buffers: &FrameBuffers,
        samplers: &Samplers,
    ) {
        self.bind_group = Self::create_bind_group(
            device,
            &self.bind_group_layout,
            &frame_buffers.color_texture_view,
            &samplers.bilinear,
        );
    }

    pub fn draw(&self, encoder: &mut wgpu::CommandEncoder, frame_buffers: &FrameBuffers) {
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Bloom Scale Down Render Pass"),
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
            rpass.set_bind_group(0, &self.bind_group, &[]);
            rpass.set_pipeline(&self.render_pipeline);
            rpass.draw(0..3, 0..1);
        }
    }
}

struct BlurPass {
    blur_bind_group_layout: wgpu::BindGroupLayout,
    blur_bind_groups: Vec<[wgpu::BindGroup; 2]>,
    blur_render_pipeline: wgpu::RenderPipeline,
}

impl BlurPass {
    pub fn new(device: &wgpu::Device, frame_buffers: &FrameBuffers, samplers: &Samplers) -> Self {
        let vertex_shader_module =
            device.create_shader_module(&wgpu::include_wgsl!("fullscreen_vs.wgsl"));

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

        let blur_bind_groups = (&frame_buffers.bloom_blur_buffers)
            .into_iter()
            .map(|buffers| {
                [
                    Self::create_blur_bind_group(
                        device,
                        &blur_bind_group_layout,
                        &buffers[0].texture_view,
                        &samplers.bilinear,
                    ),
                    Self::create_blur_bind_group(
                        device,
                        &blur_bind_group_layout,
                        &buffers[1].texture_view,
                        &samplers.bilinear,
                    ),
                ]
            })
            .collect::<Vec<_>>();

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
                    targets: &[frame_buffers.bloom_blur_buffers[0][0].format.into()],
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
            blur_bind_group_layout,
            blur_bind_groups,
            blur_render_pipeline,
        }
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

    pub fn recreate_bind_group(
        &mut self,
        device: &wgpu::Device,
        frame_buffers: &FrameBuffers,
        samplers: &Samplers,
    ) {
        self.blur_bind_groups = (&frame_buffers.bloom_blur_buffers)
            .into_iter()
            .map(|buffers| {
                [
                    Self::create_blur_bind_group(
                        device,
                        &self.blur_bind_group_layout,
                        &buffers[0].texture_view,
                        &samplers.bilinear,
                    ),
                    Self::create_blur_bind_group(
                        device,
                        &self.blur_bind_group_layout,
                        &buffers[1].texture_view,
                        &samplers.bilinear,
                    ),
                ]
            })
            .collect::<Vec<_>>();
    }

    pub fn draw(&self, encoder: &mut wgpu::CommandEncoder, frame_buffers: &FrameBuffers) {
        let attachment_views = (&frame_buffers.bloom_blur_buffers)
            .into_iter()
            .map(|buffers| [&buffers[1].texture_view, &buffers[0].texture_view]);

        for (attachment_views, bind_groups) in
            std::iter::zip(attachment_views, &self.blur_bind_groups)
        {
            for (attachment_view, bind_group) in std::iter::zip(attachment_views, bind_groups) {
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Bloom Blur Render Pass"),
                    color_attachments: &[wgpu::RenderPassColorAttachment {
                        view: attachment_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: true,
                        },
                    }],
                    depth_stencil_attachment: None,
                });
                rpass.set_bind_group(0, &bind_group, &[]);
                rpass.set_pipeline(&self.blur_render_pipeline);
                rpass.draw(0..3, 0..1);
            }
        }
    }
}
