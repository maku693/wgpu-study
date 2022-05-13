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
    position: Vec3,
    _pad0: [u8; 4],
    color: Vec3,
    _pad1: [u8; 4],
}

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

pub struct Renderer {
    surface: Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    frame_buffers: FrameBuffers,
    staging_belt: wgpu::util::StagingBelt,
    particle_uniform_buffer: wgpu::Buffer,
    particle_render_bundle: wgpu::RenderBundle,
    composite_uniform_buffer: wgpu::Buffer,
    composite_bind_group_layout: wgpu::BindGroupLayout,
    composite_bind_group: wgpu::BindGroup,
    composite_render_pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
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

        let surface_format = surface
            .get_preferred_format(&adapter)
            .context("No preferred format found")?;

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

        let PhysicalSize { width, height } = window.inner_size();

        let surface = Surface::new(surface, surface_format);
        surface.configure(&device, width, height);

        let frame_buffers = FrameBuffers::new(&device, width, height);

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let staging_belt = wgpu::util::StagingBelt::new(
            (size_of::<ParticleUniforms>() + size_of::<CompositeUniforms>()) as _,
        );

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
                    .map(|_| ParticleInstance {
                        position: vec3(
                            rng.gen_range(0.0..1.0),
                            rng.gen_range(0.0..1.0),
                            rng.gen_range(0.0..1.0),
                        ) - 0.5,
                        color: vec3(
                            rng.gen_range(0.0..1.0),
                            rng.gen_range(0.0..1.0),
                            rng.gen_range(0.0..1.0),
                        )
                        .normalize()
                            * 2.0,
                        ..Default::default()
                    })
                    .collect();
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Instance buffer"),
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

        let composite_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Composite pass uniform buffer"),
            size: size_of::<ParticleUniforms>() as _,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let composite_bind_group_layout =
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

        let composite_bind_group = Self::create_composite_bind_group(
            &device,
            &composite_bind_group_layout,
            &composite_uniform_buffer,
            &frame_buffers.color_texture_view,
            &sampler,
        );

        let composite_render_pipeline = {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&composite_bind_group_layout],
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
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            })
        };

        Ok(Self {
            surface,
            device,
            queue,
            frame_buffers,
            staging_belt,
            particle_uniform_buffer,
            particle_render_bundle,
            composite_uniform_buffer,
            composite_bind_group_layout,
            composite_bind_group,
            composite_render_pipeline,
            sampler,
        })
    }

    fn create_composite_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        uniform_buffer: &wgpu::Buffer,
        color_texture_view: &wgpu::TextureView,
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
                    resource: wgpu::BindingResource::TextureView(color_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.surface.configure(&self.device, width, height);

        self.frame_buffers.resize(&self.device, width, height);

        self.composite_bind_group = Self::create_composite_bind_group(
            &self.device,
            &self.composite_bind_group_layout,
            &self.composite_uniform_buffer,
            &self.frame_buffers.color_texture_view,
            &self.sampler,
        );
    }

    pub fn render(&mut self, scene: &Scene) -> impl Future<Output = ()> {
        let uniforms = ParticleUniforms::new(&scene);
        let composite_uniforms = CompositeUniforms::new(&scene);

        let surface_texture = self
            .surface
            .wgpu_surface
            .get_current_texture()
            .expect("Failed to get next surface texture");

        let surface_texture_view = surface_texture.texture.create_view(&Default::default());

        let mut encoder = self.device.create_command_encoder(&Default::default());

        self.staging_belt
            .write_buffer(
                &mut encoder,
                &self.particle_uniform_buffer,
                0,
                wgpu::BufferSize::new(size_of::<ParticleUniforms>() as _).unwrap(),
                &self.device,
            )
            .copy_from_slice(bytes_of(&uniforms));
        self.staging_belt
            .write_buffer(
                &mut encoder,
                &self.composite_uniform_buffer,
                0,
                wgpu::BufferSize::new(size_of::<CompositeUniforms>() as _).unwrap(),
                &self.device,
            )
            .copy_from_slice(bytes_of(&composite_uniforms));
        self.staging_belt.finish();

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &self.frame_buffers.color_texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                }],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.frame_buffers.depth_texture_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });
            render_pass.execute_bundles(Some(&self.particle_render_bundle));
        }

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &surface_texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });
            rpass.set_bind_group(0, &self.composite_bind_group, &[]);
            rpass.set_pipeline(&self.composite_render_pipeline);
            rpass.draw(0..3, 0..1);
        }

        self.queue.submit(Some(encoder.finish()));

        surface_texture.present();

        self.staging_belt.recall()
    }
}

struct Surface {
    wgpu_surface: wgpu::Surface,
    texture_format: wgpu::TextureFormat,
}

impl Surface {
    fn new(wgpu_surface: wgpu::Surface, texture_format: wgpu::TextureFormat) -> Self {
        Self {
            wgpu_surface,
            texture_format,
        }
    }

    fn configure(&self, device: &wgpu::Device, width: u32, height: u32) {
        self.wgpu_surface.configure(
            device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: self.texture_format,
                width,
                height,
                present_mode: wgpu::PresentMode::Fifo,
            },
        );
    }
}

struct FrameBuffers {
    color_texture: wgpu::Texture,
    color_texture_view: wgpu::TextureView,
    depth_texture: wgpu::Texture,
    depth_texture_view: wgpu::TextureView,
}

impl FrameBuffers {
    const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
    const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;

    fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let color_texture = Self::create_color_texture(device, width, height);
        let color_texture_view = Self::create_color_texture_view(&color_texture);

        let depth_texture = Self::create_depth_texture(device, width, height);
        let depth_texture_view = Self::create_depth_texture_view(&depth_texture);

        Self {
            color_texture,
            color_texture_view,
            depth_texture,
            depth_texture_view,
        }
    }

    fn create_color_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Offscreen Color Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::COLOR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        })
    }

    fn create_color_texture_view(texture: &wgpu::Texture) -> wgpu::TextureView {
        texture.create_view(&wgpu::TextureViewDescriptor {
            ..Default::default()
        })
    }

    fn create_depth_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        })
    }

    fn create_depth_texture_view(texture: &wgpu::Texture) -> wgpu::TextureView {
        texture.create_view(&wgpu::TextureViewDescriptor {
            aspect: wgpu::TextureAspect::DepthOnly,
            ..Default::default()
        })
    }

    fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.color_texture = Self::create_color_texture(device, width, height);
        self.color_texture_view = Self::create_color_texture_view(&self.color_texture);
        self.depth_texture = Self::create_depth_texture(device, width, height);
        self.depth_texture_view = Self::create_depth_texture_view(&self.depth_texture);
    }
}
