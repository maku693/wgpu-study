use std::{mem::size_of, time::SystemTime};

use bytemuck::{bytes_of, cast_slice, Pod, Zeroable};
use glam::{const_vec3, vec3, Mat4, Vec3, Vec4};
use log::info;
use rand::prelude::*;
use rand_pcg::Pcg64Mcg;
use wgpu::util::DeviceExt;

use crate::{component::Particle, entity::Scene};

const QUAD_VERTICES: [Vec3; 4] = [
    const_vec3!([-0.5, -0.5, 0.]),
    const_vec3!([-0.5, 0.5, 0.]),
    const_vec3!([0.5, -0.5, 0.]),
    const_vec3!([0.5, 0.5, 0.]),
];
const QUAD_INDICES: [u16; 6] = [0, 2, 1, 1, 2, 3];

#[derive(Debug, Copy, Clone, Default, Pod, Zeroable)]
#[repr(C)]
struct Uniforms {
    mv_mat: Mat4,
    p_mat: Mat4,
    particle_size: f32,
    _pad0: [u8; 12],
}

impl Uniforms {
    fn new(scene: &Scene) -> Self {
        let Scene {
            camera, particle, ..
        } = scene;

        let p_mat = {
            let fovy = camera.camera.fov / camera.camera.aspect_ratio / 180.;
            Mat4::perspective_lh(
                fovy,
                camera.camera.aspect_ratio,
                camera.camera.near,
                camera.camera.far,
            )
        };

        let v_mat = {
            let center = camera.transform.position + camera.transform.rotation * Vec3::Z;
            let up = Vec3::Y;
            Mat4::look_at_lh(camera.transform.position, center, up)
        };

        let m_mat = Mat4::from_scale_rotation_translation(
            particle.transform.scale,
            particle.transform.rotation,
            particle.transform.position,
        );

        Self {
            mv_mat: v_mat * m_mat,
            p_mat,
            particle_size: particle.particle.particle_size,
            ..Default::default()
        }
    }
}

#[derive(Debug, Copy, Clone, Default, Pod, Zeroable)]
#[repr(C)]
struct Instance {
    position: Vec4,
    color: Vec4,
}

struct Instances(Vec<Instance>);

impl Instances {
    fn as_slice(&self) -> &[Instance] {
        Vec::as_slice(&self.0)
    }
}

impl Instances {
    fn new(particle: &Particle) -> Self {
        let rand_seed = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as _;

        let mut rng = Pcg64Mcg::seed_from_u64(rand_seed);
        info!("Seeded RNG with {}", rand_seed);

        let instances = (0..particle.max_count)
            .map(|_| {
                let position = {
                    let position_range = particle.position_range;
                    let v = vec3(
                        rng.gen_range(position_range.0.x..=position_range.1.x),
                        rng.gen_range(position_range.0.y..=position_range.1.y),
                        rng.gen_range(position_range.0.z..=position_range.1.z),
                    );
                    (v, 1.0).into()
                };
                let color = {
                    let color_range = particle.color_range;
                    let v = vec3(
                        rng.gen_range(color_range.0.x..=color_range.1.x),
                        rng.gen_range(color_range.0.y..=color_range.1.y),
                        rng.gen_range(color_range.0.z..=color_range.1.z),
                    );
                    (v, 1.0).into()
                };
                Instance { position, color }
            })
            .collect::<Vec<_>>();

        Self(instances)
    }
}

pub struct ParticleRenderer {
    particle_cache: Particle,
    uniform_buffer: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    instance_count: u32,
    bind_group: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,
}

impl ParticleRenderer {
    pub fn update(&self, queue: &wgpu::Queue, scene: &Scene) {
        if self.particle_cache != scene.particle.particle {
            queue.write_buffer(
                &self.instance_buffer,
                0,
                cast_slice(&Instances::new(&scene.particle.particle).as_slice()),
            );
        }
        queue.write_buffer(&self.uniform_buffer, 0, bytes_of(&Uniforms::new(scene)));
    }

    pub fn draw<'rpass>(&'rpass self, rpass: &mut impl wgpu::util::RenderEncoder<'rpass>) {
        rpass.set_pipeline(&self.render_pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        rpass.draw_indexed(0..(QUAD_INDICES.len() as _), 0, 0..self.instance_count);
    }
}

pub struct ParticleRendererBuilder<'a> {
    scene: &'a Scene,
    color_format: Option<wgpu::TextureFormat>,
    depth_format: Option<wgpu::TextureFormat>,
}

impl<'a> ParticleRendererBuilder<'a> {
    pub fn new(scene: &'a Scene) -> Self {
        Self {
            scene,
            color_format: None,
            depth_format: None,
        }
    }

    pub fn color_target_format(mut self, format: wgpu::TextureFormat) -> Self {
        self.color_format = Some(format);
        self
    }

    pub fn depth_format(mut self, format: wgpu::TextureFormat) -> Self {
        self.depth_format = Some(format);
        self
    }

    pub fn build(self, device: &wgpu::Device) -> ParticleRenderer {
        let scene = self.scene;
        let color_format = self.color_format.expect("No color format provided");
        let depth_format = self.depth_format.expect("No depth format provided");

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

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(size_of::<Instance>() as _),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(size_of::<Uniforms>() as _),
                    },
                    count: None,
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
                targets: &[color_format.into()],
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
                format: depth_format,
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

        let particle = &scene.particle.particle;

        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Particle Instance Buffer"),
            contents: cast_slice(Instances::new(particle).as_slice()),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE,
        });
        let instance_count = particle.max_count;

        let particle_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Particle Uniform Buffer"),
            size: size_of::<Uniforms>() as _,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
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

        ParticleRenderer {
            particle_cache: particle.clone(),
            vertex_buffer,
            index_buffer,
            instance_buffer,
            instance_count,
            uniform_buffer: particle_uniform_buffer,
            bind_group,
            render_pipeline,
        }
    }
}
