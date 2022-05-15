use std::mem::size_of;

use bytemuck::{bytes_of, Pod, Zeroable};

use crate::{entity::Scene, frame_buffers::FrameBuffers, samplers::Samplers, surface::Surface};

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

pub struct CompositeRenderer {
    uniform_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,
}

impl CompositeRenderer {
    pub const STAGING_BUFFER_CHUNK_SIZE: wgpu::BufferAddress = size_of::<CompositeUniforms>() as _;

    pub fn new(
        device: &wgpu::Device,
        samplers: &Samplers,
        frame_buffers: &FrameBuffers,
        surface: &Surface,
    ) -> Self {
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Composite pass uniform buffer"),
            size: size_of::<CompositeUniforms>() as _,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout =
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
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let bind_group = Self::create_bind_group(
            device,
            &bind_group_layout,
            &uniform_buffer,
            frame_buffers,
            samplers,
        );

        let render_pipeline = {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&bind_group_layout],
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
                    targets: &[surface.texture_format.into()],
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
            uniform_buffer,
            bind_group_layout,
            bind_group,
            render_pipeline,
        }
    }

    fn create_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        uniform_buffer: &wgpu::Buffer,
        frame_buffers: &FrameBuffers,
        samplers: &Samplers,
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
                    resource: wgpu::BindingResource::TextureView(&frame_buffers.color_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &frame_buffers
                            .bloom_blur_buffers
                            .last()
                            .unwrap()
                            .texture_view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&samplers.bilinear),
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
            &self.uniform_buffer,
            frame_buffers,
            samplers,
        );
    }

    pub fn update(
        &self,
        device: &wgpu::Device,
        staging_belt: &mut wgpu::util::StagingBelt,
        encoder: &mut wgpu::CommandEncoder,
        scene: &Scene,
    ) {
        let composite_uniforms = CompositeUniforms::new(&scene);

        staging_belt
            .write_buffer(
                encoder,
                &self.uniform_buffer,
                0,
                wgpu::BufferSize::new(size_of::<CompositeUniforms>() as _).unwrap(),
                device,
            )
            .copy_from_slice(bytes_of(&composite_uniforms));
    }

    pub fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        surface_texture_view: &wgpu::TextureView,
    ) {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Composite Render Pass"),
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view: surface_texture_view,
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
