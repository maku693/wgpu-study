use std::{mem::size_of, time::SystemTime};

use bytemuck::{bytes_of, cast_slice, Pod, Zeroable};
use glam::{const_vec3, vec3, Mat4, Vec3, Vec4};
use log::info;
use rand::prelude::*;
use rand_pcg::Pcg64Mcg;
use wgpu::util::DeviceExt;

use crate::{entity::Scene, frame_buffers::FrameBuffers};

const QUAD_VERTICES: [Vec3; 4] = [
    const_vec3!([-0.5, -0.5, 0.]),
    const_vec3!([-0.5, 0.5, 0.]),
    const_vec3!([0.5, -0.5, 0.]),
    const_vec3!([0.5, 0.5, 0.]),
];
const QUAD_INDICES: [u16; 6] = [0, 2, 1, 1, 2, 3];

#[derive(Debug, Copy, Clone, Default, Pod, Zeroable)]
#[repr(C)]
struct ParticleUniforms {
    mv_mat: Mat4,
    p_mat: Mat4,
    particle_size: f32,
    _pad0: [u8; 12],
}

impl ParticleUniforms {
    fn new(scene: &Scene) -> Self {
        let Scene {
            camera,
            particle_system,
            ..
        } = scene;

        let p_mat = {
            let fovy = camera.fov / camera.aspect_ratio / 180.;
            Mat4::perspective_lh(fovy, camera.aspect_ratio, camera.near, camera.far)
        };

        let v_mat = {
            let center = camera.transform.position + camera.transform.rotation * Vec3::Z;
            let up = Vec3::Y;
            Mat4::look_at_lh(camera.transform.position, center, up)
        };

        let m_mat = Mat4::from_scale_rotation_translation(
            particle_system.transform.scale,
            particle_system.transform.rotation,
            particle_system.transform.position,
        );

        Self {
            mv_mat: v_mat * m_mat,
            p_mat,
            particle_size: particle_system.particle_size,
            ..Default::default()
        }
    }
}

#[derive(Debug, Copy, Clone, Default, Pod, Zeroable)]
#[repr(C)]
struct ParticleInstance {
    position: Vec4,
    color: Vec4,
}

pub struct ParticleRenderer {
    particle_uniform_buffer: wgpu::Buffer,
    particle_render_bundle: wgpu::RenderBundle,
}

impl ParticleRenderer {
    pub const STAGING_BUFFER_CHUNK_SIZE: wgpu::BufferAddress = size_of::<ParticleUniforms>() as _;

    pub fn new(device: &wgpu::Device, scene: &Scene) -> Self {
        let particle_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Uniform Buffer"),
            size: size_of::<ParticleUniforms>() as _,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let particle_render_bundle = {
            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Particle Vertex Buffer"),
                contents: bytes_of(&QUAD_VERTICES),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Particle Index Buffer"),
                contents: bytes_of(&QUAD_INDICES),
                usage: wgpu::BufferUsages::INDEX,
            });

            let instance_buffer = {
                let rand_seed = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as _;

                let mut rng = Pcg64Mcg::seed_from_u64(rand_seed);
                info!("Seeded RNG with {}", rand_seed);

                let instances: Vec<_> = (0..scene.particle_system.max_count)
                    .map(|_| {
                        let position = {
                            let mut v = vec3(
                                rng.gen_range(0.0..1.0),
                                rng.gen_range(0.0..1.0),
                                rng.gen_range(0.0..1.0),
                            );
                            v -= 0.5;

                            (v, 1.0).into()
                        };
                        let color = {
                            let mut v = vec3(
                                rng.gen_range(0.0..1.0),
                                rng.gen_range(0.0..1.0),
                                rng.gen_range(0.0..1.0),
                            );
                            v = v.normalize();
                            v *= 2.0;

                            (v, 1.0).into()
                        };
                        ParticleInstance { position, color }
                    })
                    .collect();

                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Instance Buffer"),
                    contents: cast_slice(instances.as_slice()),
                    usage: wgpu::BufferUsages::STORAGE,
                })
            };

            let bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::VERTEX,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(
                                    size_of::<ParticleInstance>() as _,
                                ),
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::VERTEX,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(
                                    size_of::<ParticleUniforms>() as _,
                                ),
                            },
                            count: None,
                        },
                    ],
                });

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: instance_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: particle_uniform_buffer.as_entire_binding(),
                    },
                ],
            });

            let shader_module = device.create_shader_module(&wgpu::include_wgsl!("particle.wgsl"));

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

            let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                    targets: &[FrameBuffers::COLOR_FORMAT.into()],
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
                    format: FrameBuffers::DEPTH_FORMAT,
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
            });

            let mut encoder =
                device.create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
                    label: None,
                    color_formats: &[FrameBuffers::COLOR_FORMAT],
                    depth_stencil: Some(wgpu::RenderBundleDepthStencil {
                        format: FrameBuffers::DEPTH_FORMAT,
                        depth_read_only: false,
                        stencil_read_only: true,
                    }),
                    sample_count: 1,
                    multiview: None,
                });

            encoder.set_bind_group(0, &bind_group, &[]);
            encoder.set_pipeline(&render_pipeline);
            encoder.set_vertex_buffer(0, vertex_buffer.slice(..));
            encoder.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            encoder.draw_indexed(
                0..(QUAD_INDICES.len() as _),
                0,
                0..scene.particle_system.max_count,
            );

            encoder.finish(&wgpu::RenderBundleDescriptor {
                label: Some("Particle Render Bundle"),
            })
        };

        Self {
            particle_uniform_buffer,
            particle_render_bundle,
        }
    }

    pub fn update(
        &self,
        device: &wgpu::Device,
        staging_belt: &mut wgpu::util::StagingBelt,
        encoder: &mut wgpu::CommandEncoder,
        scene: &Scene,
    ) {
        let uniforms = ParticleUniforms::new(&scene);

        staging_belt
            .write_buffer(
                encoder,
                &self.particle_uniform_buffer,
                0,
                wgpu::BufferSize::new(size_of::<ParticleUniforms>() as _).unwrap(),
                device,
            )
            .copy_from_slice(bytes_of(&uniforms));
    }

    pub fn draw(&self, encoder: &mut wgpu::CommandEncoder, frame_buffers: &FrameBuffers) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view: &frame_buffers.color_texture_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: true,
                },
            }],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &frame_buffers.depth_texture_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });
        render_pass.execute_bundles(Some(&self.particle_render_bundle));
    }
}
