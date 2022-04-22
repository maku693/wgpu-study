use std::{mem::size_of, time::SystemTime};

use anyhow::Result;
use bytemuck::{bytes_of, cast_slice, Pod, Zeroable};
use glam::{const_vec3, vec3, Mat4, Vec3};
use log::{debug, info};
use rand::prelude::*;
use rand_pcg::Pcg64Mcg;
use wgpu::util::DeviceExt;

use crate::renderer;

use super::entity;

trait CameraExt {
    fn proj_matrix(&self) -> Mat4;
    fn view_matrix(&self) -> Mat4;
}

impl CameraExt for entity::Camera {
    fn proj_matrix(&self) -> Mat4 {
        let fovy = self.fov / self.aspect_ratio / 180.;
        Mat4::perspective_lh(fovy, self.aspect_ratio, self.near, self.far)
    }

    fn view_matrix(&self) -> Mat4 {
        let center = self.position + self.rotation * Vec3::Z;
        let up = Vec3::Y;
        Mat4::look_at_lh(self.position, center, up)
    }
}

trait ParticleSystemExt {
    fn model_matrix(&self) -> Mat4;
}

impl ParticleSystemExt for entity::ParticleSystem {
    fn model_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
    }
}

#[derive(Debug, Copy, Clone, Default, Pod, Zeroable)]
#[repr(C)]
struct Uniforms {
    mvp_matrix: Mat4,
}

impl Uniforms {
    fn new(scene: &entity::Scene) -> Self {
        Self {
            mvp_matrix: scene.camera.proj_matrix()
                * scene.camera.view_matrix()
                * scene.particle_system.model_matrix(),
        }
    }
}

#[derive(Debug, Copy, Clone, Default, Pod, Zeroable)]
#[repr(C)]
struct Instance {
    position: Vec3,
    _pad0: [u8; 4],
    color: Vec3,
    _pad1: [u8; 4],
}

pub struct PipelineState {
    uniform_buffer: wgpu::Buffer,
    _vertex_buffer: wgpu::Buffer,
    _index_buffer: wgpu::Buffer,
    _instance_buffer: wgpu::Buffer,

    render_bundle: wgpu::RenderBundle,
}

impl PipelineState {
    const PARTICLE_VERTICES: [Vec3; 4] = [
        const_vec3!([-0.5, -0.5, 0.]),
        const_vec3!([-0.5, 0.5, 0.]),
        const_vec3!([0.5, -0.5, 0.]),
        const_vec3!([0.5, 0.5, 0.]),
    ];
    const PARTICLE_INDICES: [u16; 6] = [0, 2, 1, 1, 2, 3];

    pub fn new(
        device: &wgpu::Device,
        render_target_color_format: wgpu::TextureFormat,
        scene: &entity::Scene,
    ) -> Self {
        let uniform_buffer = Self::make_uniform_buffer(device, scene);
        let vertex_buffer = Self::make_vertex_buffer(device);
        let index_buffer = Self::make_index_buffer(device);
        let instance_buffer = Self::make_instance_buffer(device, scene);

        let bind_group_layout = Self::make_bind_group_layout(device);
        let bind_group = Self::make_bind_group(
            device,
            &bind_group_layout,
            &uniform_buffer,
            &instance_buffer,
        );
        let render_pipeline =
            Self::make_render_pipeline(device, render_target_color_format, &bind_group_layout);

        let render_bundle = Self::make_render_bundle(
            device,
            render_target_color_format,
            &render_pipeline,
            &bind_group,
            &vertex_buffer,
            &index_buffer,
            scene,
        );

        Self {
            uniform_buffer,
            _vertex_buffer: vertex_buffer,
            _index_buffer: index_buffer,
            _instance_buffer: instance_buffer,
            render_bundle,
        }
    }

    fn make_vertex_buffer(device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex buffer"),
            contents: bytes_of(&Self::PARTICLE_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        })
    }

    fn make_index_buffer(device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index buffer"),
            contents: bytes_of(&Self::PARTICLE_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        })
    }

    fn make_instance_buffer(device: &wgpu::Device, scene: &entity::Scene) -> wgpu::Buffer {
        let unix_milli = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as _;
        info!("Seeded RNG with {}", unix_milli);
        let mut rng = Pcg64Mcg::seed_from_u64(unix_milli);

        let instances: Vec<_> = (0..scene.particle_system.max_count)
            .map(|_| Instance {
                position: vec3(
                    rng.gen_range(-1.0..1.0) * 100.0,
                    rng.gen_range(-1.0..1.0) * 100.0,
                    rng.gen_range(-1.0..1.0) * 100.0,
                ),
                color: vec3(
                    rng.gen_range(0.5..1.0),
                    rng.gen_range(0.5..1.0),
                    rng.gen_range(0.5..1.0),
                ),
                ..Default::default()
            })
            .collect();
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance buffer"),
            contents: cast_slice(instances.as_slice()),
            usage: wgpu::BufferUsages::STORAGE,
        })
    }

    fn make_uniform_buffer(device: &wgpu::Device, scene: &entity::Scene) -> wgpu::Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform buffer"),
            contents: bytes_of(&Uniforms::new(scene)),
            usage: wgpu::BufferUsages::UNIFORM
                | wgpu::BufferUsages::MAP_READ
                | wgpu::BufferUsages::MAP_WRITE,
        })
    }

    fn make_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(size_of::<Uniforms>() as _),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(size_of::<Instance>() as _),
                    },
                    count: None,
                },
            ],
        })
    }

    fn make_bind_group(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        uniform_buffer: &wgpu::Buffer,
        instance_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: instance_buffer.as_entire_binding(),
                },
            ],
        })
    }

    fn make_render_pipeline(
        device: &wgpu::Device,
        render_target_format: wgpu::TextureFormat,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> wgpu::RenderPipeline {
        let shader_module = device.create_shader_module(&wgpu::include_wgsl!("main.wgsl"));

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[bind_group_layout],
            push_constant_ranges: &[],
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: size_of::<Vec3>() as _,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0,
                    }],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: "fs_main",
                targets: &[render_target_format.into()],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        })
    }

    fn make_render_bundle(
        device: &wgpu::Device,
        render_target_color_format: wgpu::TextureFormat,
        render_pipeline: &wgpu::RenderPipeline,
        bind_group: &wgpu::BindGroup,
        vertex_buffer: &wgpu::Buffer,
        index_buffer: &wgpu::Buffer,
        scene: &entity::Scene,
    ) -> wgpu::RenderBundle {
        let mut encoder =
            device.create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
                label: None,
                color_formats: &[render_target_color_format],
                depth_stencil: None,
                sample_count: 1,
                multiview: None,
            });

        encoder.set_pipeline(render_pipeline);
        encoder.set_bind_group(0, bind_group, &[]);
        encoder.set_vertex_buffer(0, vertex_buffer.slice(..));
        encoder.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        encoder.draw_indexed(
            0..(Self::PARTICLE_INDICES.len() as _),
            0,
            0..scene.particle_system.max_count,
        );

        encoder.finish(&wgpu::RenderBundleDescriptor { label: None })
    }

    pub async fn update(&self, scene: &entity::Scene) -> Result<()> {
        let uniforms = Uniforms::new(scene);
        debug!("{:#?}", uniforms);

        let uniform_buffer_slice = self.uniform_buffer.slice(..);
        uniform_buffer_slice.map_async(wgpu::MapMode::Write).await?;
        uniform_buffer_slice
            .get_mapped_range_mut()
            .copy_from_slice(bytes_of(&uniforms));
        self.uniform_buffer.unmap();

        Ok(())
    }
}

impl renderer::Pipeline for PipelineState {
    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.execute_bundles(Some(&self.render_bundle));
    }
}
