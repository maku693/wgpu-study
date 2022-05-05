use std::{future::Future, mem::size_of, time::SystemTime};

use anyhow::{Context, Ok, Result};
use bytemuck::{bytes_of, cast_slice, Pod, Zeroable};
use glam::{const_vec3, vec3, Mat4, Vec3};
use log::info;
use rand::prelude::*;
use rand_pcg::Pcg64Mcg;
use wgpu::util::DeviceExt;
use winit::{dpi::PhysicalSize, window::Window};

use crate::entity::Scene;

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

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
struct Instance {
    position: Vec3,
    _pad0: [u8; 4],
    color: Vec3,
    _pad1: [u8; 4],
}

pub struct Renderer {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_format: wgpu::TextureFormat,
    depth_texture_view: wgpu::TextureView,
    staging_belt: wgpu::util::StagingBelt,
    uniform_buffer: wgpu::Buffer,
    render_bundle: wgpu::RenderBundle,
}

impl Renderer {
    pub async fn new(window: &Window, scene: &Scene) -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);
        let surface = unsafe { instance.create_surface(&window) };

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("No adapter found")?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                },
                None,
            )
            .await?;

        let surface_format = surface
            .get_preferred_format(&adapter)
            .context("No preferred format found")?;

        let PhysicalSize { width, height } = window.inner_size();

        configure_surface(&device, &surface, surface_format, width, height);
        let depth_texture_view = create_depth_texture_view(&device, width, height);

        let staging_belt = wgpu::util::StagingBelt::new(64);

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Uniform buffer"),
            size: size_of::<Uniforms>() as _,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Particle vertex buffer"),
            contents: bytes_of(&QUAD_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Particle index buffer"),
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
                .map(|_| Instance {
                    position: vec3(
                        rng.gen_range(-0.5..0.5),
                        rng.gen_range(-0.5..0.5),
                        rng.gen_range(-0.5..0.5),
                    ) * scene.particle_system.transform.scale,
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
        };

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
                    resource: uniform_buffer.as_entire_binding(),
                },
            ],
        });
        let shader_module = device.create_shader_module(&wgpu::include_wgsl!("main.wgsl"));

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
                targets: &[surface_format.into()],
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
                format: DEPTH_FORMAT,
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
                color_formats: &[surface_format],
                depth_stencil: Some(wgpu::RenderBundleDepthStencil {
                    format: DEPTH_FORMAT,
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

        let render_bundle = encoder.finish(&wgpu::RenderBundleDescriptor {
            label: Some("Particle render bundle"),
        });

        Ok(Self {
            surface,
            device,
            queue,
            surface_format,
            staging_belt,
            uniform_buffer,
            depth_texture_view,
            render_bundle,
        })
    }

    pub fn recreate_render_attachments(
        &mut self,
        PhysicalSize { width, height }: PhysicalSize<u32>,
    ) {
        configure_surface(
            &self.device,
            &self.surface,
            self.surface_format,
            width,
            height,
        );

        self.depth_texture_view = create_depth_texture_view(&self.device, width, height)
    }

    pub fn render(&mut self, scene: &Scene) -> impl Future<Output = ()> {
        let uniforms = Uniforms::new(&scene);

        let frame_buffer = self
            .surface
            .get_current_texture()
            .expect("Failed to get next surface texture");

        let frame_buffer_view = frame_buffer.texture.create_view(&Default::default());

        let mut encoder = self.device.create_command_encoder(&Default::default());

        self.staging_belt
            .write_buffer(
                &mut encoder,
                &self.uniform_buffer,
                0,
                wgpu::BufferSize::new(size_of::<Uniforms>() as _).unwrap(),
                &self.device,
            )
            .copy_from_slice(bytes_of(&uniforms));
        self.staging_belt.finish();

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &frame_buffer_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                }],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });

            render_pass.execute_bundles(Some(&self.render_bundle));
        }

        self.queue.submit(Some(encoder.finish()));

        frame_buffer.present();

        self.staging_belt.recall()
    }
}

fn configure_surface(
    device: &wgpu::Device,
    surface: &wgpu::Surface,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
) {
    surface.configure(
        device,
        &wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
        },
    );
}

fn create_depth_texture_view(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Depth texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
    });

    depth_texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("Depth texture view"),
        format: Some(DEPTH_FORMAT),
        dimension: Some(wgpu::TextureViewDimension::D2),
        aspect: wgpu::TextureAspect::DepthOnly,
        base_mip_level: 0,
        mip_level_count: None,
        base_array_layer: 0,
        array_layer_count: None,
    })
}
