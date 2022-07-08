use std::mem::size_of;

use bytemuck::{bytes_of, Pod, Zeroable};
use glam::{vec2, Vec2};
use wgpu::util::DeviceExt;

use crate::renderer::wgpu_ext;

#[derive(Debug, Copy, Clone, PartialEq, Default, Pod, Zeroable)]
#[repr(C)]
struct Uniforms {
    resolution: Vec2,
}

pub struct BlurUpsampleRenderPass {
    render_pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
}

impl BlurUpsampleRenderPass {
    pub fn new(
        device: &wgpu::Device,
        src_texture: &wgpu_ext::Texture,
        render_target_texture: &wgpu_ext::Texture,
    ) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Blur Upsample Bilinear Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
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
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(size_of::<Uniforms>() as _),
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
                device.create_shader_module(&wgpu::include_wgsl!("blur_upsample.fragment.wgsl"));

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
                    targets: &[render_target_texture.format().into()],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            })
        };

        let bind_group = {
            let src_texture_view = src_texture
                .as_ref()
                .create_view(&wgpu::TextureViewDescriptor::default());

            let resolution = vec2(
                render_target_texture.width() as _,
                render_target_texture.height() as _,
            );
            let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Blur Downsample Uniform Buffer"),
                contents: bytes_of(&Uniforms { resolution }),
                usage: wgpu::BufferUsages::UNIFORM,
            });

            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&src_texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                ],
            })
        };

        Self {
            render_pipeline,
            bind_group,
        }
    }

    pub fn draw<'rpass>(&'rpass self, rpass: &mut impl wgpu::util::RenderEncoder<'rpass>) {
        rpass.set_pipeline(&self.render_pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.draw(0..3, 0..1);
    }
}
