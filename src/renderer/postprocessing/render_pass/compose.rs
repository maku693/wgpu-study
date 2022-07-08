use std::mem::size_of;

use bytemuck::{bytes_of, Pod, Zeroable};

use crate::entity::Scene;

#[derive(Debug, Copy, Clone, PartialEq, Default, Pod, Zeroable)]
#[repr(C)]
struct Uniforms {
    exposure: f32,
}

impl Uniforms {
    fn new(scene: &Scene) -> Self {
        Self {
            exposure: scene.camera.camera.exposure,
        }
    }
}

pub struct ComposeRenderPass {
    uniform_buffer: wgpu::Buffer,
    render_pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
}

impl ComposeRenderPass {
    pub fn new(
        device: &wgpu::Device,
        color_texture: &wgpu::Texture,
        bloom_texture: &wgpu::Texture,
        render_target_format: wgpu::TextureFormat,
    ) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Compose Render Pass Bilinear Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Bright Pass Uniform Buffer"),
            size: size_of::<Uniforms>() as _,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(size_of::<Uniforms>() as _),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
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
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let render_pipeline = {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

            let vertex_shader_module =
                device.create_shader_module(&wgpu::include_wgsl!("fullscreen.vertex.wgsl"));
            let fragment_shader_module =
                device.create_shader_module(&wgpu::include_wgsl!("compose.fragment.wgsl"));

            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Post Process Compose Render Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vertex_shader_module,
                    entry_point: "main",
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &fragment_shader_module,
                    entry_point: "main",
                    targets: &[render_target_format.into()],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            })
        };

        let color_texture_view = color_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bloom_texture_view = bloom_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&color_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&bloom_texture_view),
                },
            ],
        });

        Self {
            uniform_buffer,
            render_pipeline,
            bind_group,
        }
    }

    pub fn update(&self, queue: &wgpu::Queue, scene: &Scene) {
        queue.write_buffer(&self.uniform_buffer, 0, bytes_of(&Uniforms::new(scene)));
    }

    pub fn draw<'rpass>(&'rpass self, rpass: &mut impl wgpu::util::RenderEncoder<'rpass>) {
        rpass.set_pipeline(&self.render_pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.draw(0..3, 0..1);
    }
}
