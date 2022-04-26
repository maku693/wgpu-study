use std::{mem::size_of, time::SystemTime};

use anyhow::Result;
use bytemuck::{bytes_of, cast_slice, Pod, Zeroable};
use glam::{const_vec3, vec3, Mat4, Vec3};
use log::{debug, info};
use rand::prelude::*;
use rand_pcg::Pcg64Mcg;
use wgpu::util::DeviceExt;

use crate::{entity, renderer};

#[derive(Debug, Copy, Clone, Default, Pod, Zeroable)]
#[repr(C)]
struct Uniforms {
    mvp_matrix: Mat4,
    m_mat: Mat4,
    v_mat: Mat4,
    p_mat: Mat4,
}

impl Uniforms {
    fn new(scene: &entity::Scene) -> Self {
        let entity::Scene {
            camera,
            particle_system,
            ..
        } = scene;

        let p_mat = {
            let fovy = camera.fov / camera.aspect_ratio / 180.;
            Mat4::perspective_lh(fovy, camera.aspect_ratio, camera.near, camera.far)
        };

        let v_mat = {
            let center = camera.position + camera.rotation * Vec3::Z;
            let up = Vec3::Y;
            Mat4::look_at_lh(camera.position, center, up)
        };

        let m_mat = Mat4::from_scale_rotation_translation(
            particle_system.scale,
            particle_system.rotation,
            particle_system.position,
        );

        Self {
            mvp_matrix: p_mat * v_mat * m_mat,
            m_mat,
            v_mat,
            p_mat,
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
        render_target_depth_format: wgpu::TextureFormat,
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
        let render_pipeline = Self::make_render_pipeline(
            device,
            &bind_group_layout,
            render_target_color_format,
            render_target_depth_format,
        );

        let render_bundle = Self::make_render_bundle(
            device,
            render_target_color_format,
            render_target_depth_format,
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
                    rng.gen_range(-1.0..1.0),
                    rng.gen_range(-1.0..1.0),
                    rng.gen_range(-1.0..1.0),
                ) * 100.0,
                color: vec3(
                    rng.gen_range(0.0..1.0),
                    rng.gen_range(0.0..1.0),
                    rng.gen_range(0.0..1.0),
                )
                .normalize(),
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
        bind_group_layout: &wgpu::BindGroupLayout,
        render_target_color_format: wgpu::TextureFormat,
        render_target_depth_format: wgpu::TextureFormat,
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
                targets: &[render_target_color_format.into()],
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
            depth_stencil: Some(wgpu::DepthStencilState {
                format: render_target_depth_format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: 0,
                    slope_scale: 0.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        })
    }

    fn make_render_bundle(
        device: &wgpu::Device,
        render_target_color_format: wgpu::TextureFormat,
        render_target_depth_format: wgpu::TextureFormat,
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
                depth_stencil: Some(wgpu::RenderBundleDepthStencil {
                    format: render_target_depth_format,
                    depth_read_only: false,
                    stencil_read_only: true,
                }),
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
