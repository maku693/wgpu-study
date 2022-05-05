use std::{
    f32::consts::PI,
    mem::size_of,
    time::{Duration, Instant, SystemTime},
};

use anyhow::{Context, Result};
use bytemuck::{bytes_of, cast_slice, Pod, Zeroable};
use glam::{const_vec3, vec3, EulerRot, Mat4, Quat, Vec3};
use log::{debug, info};
use rand::{Rng, SeedableRng};
use rand_pcg::Pcg64Mcg;
use smol::{block_on, LocalExecutor};
use wgpu::util::DeviceExt;
use winit::{
    dpi::PhysicalPosition,
    event::{
        DeviceEvent, ElementState, Event, KeyboardInput, MouseButton, MouseScrollDelta,
        VirtualKeyCode, WindowEvent,
    },
    event_loop::{ControlFlow, EventLoop},
};

#[derive(Debug, Copy, Clone, Default)]
pub struct Scene {
    pub camera: Camera,
    pub particle_system: ParticleSystem,
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Camera {
    pub transform: Transform,
    pub fov: f32,
    pub aspect_ratio: f32,
    pub near: f32,
    pub far: f32,
}

#[derive(Debug, Copy, Clone, Default)]
pub struct ParticleSystem {
    pub transform: Transform,
    pub max_count: u32,
    pub particle_size: f32,
    pub lifetime: u32,
    pub min_speed: f32,
    pub max_speed: f32,
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

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

fn main() -> Result<()> {
    env_logger::init();

    let executer = LocalExecutor::new();

    let event_loop = EventLoop::new();

    let window = winit::window::WindowBuilder::new()
        .with_title("wgpu example")
        .with_inner_size(winit::dpi::LogicalSize::<u32> {
            width: 640,
            height: 360,
        })
        .build(&event_loop)?;

    let mut cursor_locked = false;
    let mut last_drawn_at = Instant::now();

    let mut scene = Scene {
        camera: {
            let inner_size = window.inner_size();
            let aspect_ratio = inner_size.width as f32 / inner_size.height as f32;
            Camera {
                transform: Transform {
                    position: Vec3::ZERO,
                    rotation: Quat::IDENTITY,
                    ..Default::default()
                },
                fov: 60.,
                aspect_ratio,
                near: 0.1,
                far: 1000.,
            }
        },
        particle_system: ParticleSystem {
            transform: Transform {
                position: vec3(0., 0., 10.),
                rotation: Quat::from_axis_angle(Vec3::X, PI * -0.25),
                scale: Vec3::ONE * 1.5,
            },
            max_count: 10000,
            particle_size: 0.01,
            lifetime: 0,
            min_speed: 0.01,
            max_speed: 1.,
        },
    };
    info!("{:#?}", &scene);

    let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);
    let surface = unsafe { instance.create_surface(&window) };

    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::default(),
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    }))
    .context("No adapter found")?;

    let (device, queue) = block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: None,
            features: wgpu::Features::empty(),
            limits: wgpu::Limits::default(),
        },
        None,
    ))?;

    let winit::dpi::PhysicalSize { width, height } = window.inner_size();

    let surface_format = surface
        .get_preferred_format(&adapter)
        .context("There is no preferred format")?;
    configure_surface(&surface, &device, surface_format, width, height);

    let mut depth_texture_view = create_depth_texture_view(&device, DEPTH_FORMAT, width, height);

    let mut staging_belt = wgpu::util::StagingBelt::new(64);

    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Uniform buffer"),
        size: size_of::<Uniforms>() as _,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let render_bundle = {
        let particle_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Particle vertex buffer"),
            contents: bytes_of(&QUAD_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let particle_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Particle index buffer"),
            contents: bytes_of(&QUAD_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        let instance_buffer = {
            let unix_milli = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as _;

            let mut rng = Pcg64Mcg::seed_from_u64(unix_milli);
            info!("Seeded RNG with {}", unix_milli);

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

        let render_pipeline = {
            let shader_module = device.create_shader_module(&wgpu::include_wgsl!("main.wgsl"));

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&bind_group_layout],
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
            })
        };

        encoder.set_bind_group(0, &bind_group, &[]);
        encoder.set_pipeline(&render_pipeline);
        encoder.set_vertex_buffer(0, particle_vertex_buffer.slice(..));
        encoder.set_index_buffer(particle_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        encoder.draw_indexed(
            0..(QUAD_INDICES.len() as _),
            0,
            0..scene.particle_system.max_count,
        );

        encoder.finish(&wgpu::RenderBundleDescriptor {
            label: Some("Particle render bundle"),
        })
    };

    event_loop.run(move |e, _, control_flow| {
        debug!("{:#?}", e);

        match e {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::Resized(size) => {
                    depth_texture_view = resize(&device, &surface, surface_format, size);
                    scene.camera.aspect_ratio = size.width as f32 / size.height as f32;
                }
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    depth_texture_view = resize(&device, &surface, surface_format, *new_inner_size);
                    scene.camera.aspect_ratio =
                        new_inner_size.width as f32 / new_inner_size.height as f32;
                }
                WindowEvent::MouseInput {
                    state: ElementState::Released,
                    button: MouseButton::Left,
                    ..
                } => {
                    window.set_cursor_grab(true).unwrap();
                    window.set_cursor_visible(false);
                    cursor_locked = true;
                }
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            state: ElementState::Released,
                            virtual_keycode,
                            ..
                        },
                    ..
                } => match virtual_keycode {
                    Some(VirtualKeyCode::Escape) => {
                        window.set_cursor_grab(false).unwrap();
                        window.set_cursor_visible(true);
                        cursor_locked = false;
                    }
                    _ => (),
                },
                _ => (),
            },
            Event::DeviceEvent { event, .. } => match event {
                DeviceEvent::MouseMotion { delta: (x, y) } => {
                    if !cursor_locked {
                        return;
                    };
                    let mut rotation = scene.camera.transform.rotation.to_euler(EulerRot::YXZ);
                    rotation.0 += x as f32 * 0.001;
                    rotation.1 = (rotation.1 + y as f32 * 0.001).clamp(PI * -0.5, PI * 0.5);
                    debug!("rotation: {:?}", rotation);
                    scene.camera.transform.rotation =
                        Quat::from_euler(glam::EulerRot::YXZ, rotation.0, rotation.1, rotation.2);
                }
                DeviceEvent::MouseWheel { delta } => {
                    if !cursor_locked {
                        return;
                    };
                    let y = match delta {
                        MouseScrollDelta::PixelDelta(PhysicalPosition { y, .. }) => y as f32,
                        MouseScrollDelta::LineDelta(_, y) => y * 60.0,
                    };

                    scene.camera.fov = (scene.camera.fov + y * -0.1).clamp(30., 120.);
                    debug!("fov: {:?}", scene.camera.fov);
                }
                _ => (),
            },
            Event::MainEventsCleared => {
                while executer.try_tick() {}

                let target_frame_interval = Duration::from_secs_f64(1.0 / 60.0);
                let elapsed_from_last_draw = last_drawn_at.elapsed();
                if target_frame_interval > elapsed_from_last_draw {
                    let wait = target_frame_interval - elapsed_from_last_draw;
                    *control_flow = ControlFlow::WaitUntil(Instant::now() + wait);
                    return;
                }

                last_drawn_at = Instant::now();

                scene.particle_system.transform.rotation *=
                    Quat::from_axis_angle(Vec3::Y, PI * 0.01);

                let uniforms = Uniforms::new(&scene);

                let frame_buffer = surface
                    .get_current_texture()
                    .expect("Failed to get next surface texture");

                let frame_buffer_view = frame_buffer.texture.create_view(&Default::default());

                let mut encoder = device.create_command_encoder(&Default::default());

                staging_belt
                    .write_buffer(
                        &mut encoder,
                        &uniform_buffer,
                        0,
                        wgpu::BufferSize::new(size_of::<Uniforms>() as _).unwrap(),
                        &device,
                    )
                    .copy_from_slice(bytes_of(&uniforms));
                staging_belt.finish();

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
                            view: &depth_texture_view,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.0),
                                store: true,
                            }),
                            stencil_ops: None,
                        }),
                    });

                    render_pass.execute_bundles(Some(&render_bundle));
                }

                queue.submit(Some(encoder.finish()));

                frame_buffer.present();

                let fut = staging_belt.recall();
                executer.spawn(fut).detach();
            }
            _ => (),
        }
    });
}

fn configure_surface(
    surface: &wgpu::Surface,
    device: &wgpu::Device,
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

fn create_depth_texture_view(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        })
        .create_view(&wgpu::TextureViewDescriptor {
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

fn resize(
    device: &wgpu::Device,
    surface: &wgpu::Surface,
    surface_format: wgpu::TextureFormat,
    size: winit::dpi::PhysicalSize<u32>,
) -> wgpu::TextureView {
    let depth_texture_view =
        create_depth_texture_view(device, DEPTH_FORMAT, size.width, size.height);
    configure_surface(surface, device, surface_format, size.width, size.height);
    depth_texture_view
}
