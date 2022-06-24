use std::mem::size_of;

use bytemuck::{bytes_of, Pod, Zeroable};

use crate::{entity::Scene, window::Size};

#[derive(Debug, Copy, Clone, PartialEq, Default, Pod, Zeroable)]
#[repr(C)]
struct Uniforms {
    intensity: f32,
    threshold: f32,
}

impl Uniforms {
    fn new(scene: &Scene) -> Self {
        Self {
            intensity: scene.post_processing.bloom.intensity,
            threshold: scene.post_processing.bloom.threshold,
        }
    }
}

pub struct BrightPassRenderer {
    src_texture_size: Size,
    src_texture: wgpu::Texture,
    uniform_buffer: wgpu::Buffer,
    render_pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
}

impl BrightPassRenderer {
    pub fn new(
        device: &wgpu::Device,
        src_texture_size: Size,
        src_texture_format: wgpu::TextureFormat,
        color_target_format: wgpu::TextureFormat,
    ) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Bright Pass Bilinear Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let src_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Bright Pass Source Texture"),
            size: src_texture_size.into(),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: src_texture_format,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        });
        let src_texture_view = src_texture.create_view(&wgpu::TextureViewDescriptor::default());

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
            ],
        });

        let render_pipeline = {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

            let vertex_shader_module =
                device.create_shader_module(&wgpu::include_wgsl!("vs_fullscreen.wgsl"));

            let fragment_shader_module =
                device.create_shader_module(&wgpu::include_wgsl!("fs_bright_pass.wgsl"));

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
                    targets: &[color_target_format.into()],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            })
        };

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
                    resource: wgpu::BindingResource::TextureView(&src_texture_view),
                },
            ],
        });

        Self {
            src_texture_size,
            src_texture,
            uniform_buffer,
            render_pipeline,
            bind_group,
        }
    }

    pub fn update(&self, device: &wgpu::Device, queue: &wgpu::Queue, scene: &Scene) {
        queue.write_buffer(&self.uniform_buffer, 0, bytes_of(&Uniforms::new(scene)));
    }

    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        src_texture: &wgpu::Texture,
        color_attachment_view: &wgpu::TextureView,
    ) {
        encoder.copy_texture_to_texture(
            src_texture.as_image_copy(),
            self.src_texture.as_image_copy(),
            self.src_texture_size.into(),
        );
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Bright Pass Render Pass"),
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view: color_attachment_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: true,
                },
            }],
            depth_stencil_attachment: None,
        });
        self.draw(&mut rpass);
    }

    fn draw<'rpass>(&'rpass self, rpass: &mut impl wgpu::util::RenderEncoder<'rpass>) {
        rpass.set_pipeline(&self.render_pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.draw(0..3, 0..1);
    }
}
