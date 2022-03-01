use anyhow::{Context, Result};
use glam::{vec3, Mat4};
use pollster::FutureExt as _;
use wgpu::util::DeviceExt;
use winit;

pub struct Renderer {
    surface: wgpu::Surface,
    surface_format: wgpu::TextureFormat,
    surface_size: wgpu::Extent3d,
    device: wgpu::Device,
    queue: wgpu::Queue,
    render_target: wgpu::Texture,
    num_vertices: u32,
    vertex_buffer: wgpu::Buffer,
    num_instances: u32,
    instance_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,
}

impl Renderer {
    pub fn new(window: &winit::window::Window) -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);
        let surface = unsafe { instance.create_surface(&window) };

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .block_on()
            .context("No adapter found")?;

        let surface_format = surface
            .get_preferred_format(&adapter)
            .context("Surface is incompatible with the adapter")?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                },
                None,
            )
            .block_on()
            .context("No device found")?;

        let window_size = window.inner_size();

        configure_surface(&surface, &device, surface_format, window_size);

        let surface_size = wgpu::Extent3d {
            width: window_size.width,
            height: window_size.height,
            depth_or_array_layers: 1,
        };
        let render_target = create_render_target(&device, surface_size, surface_format);

        let vertices = [
            vec3(-0.1f32, -0.1, 0.),
            vec3(0., 0.1, 0.),
            vec3(0.1, -0.1, 0.),
        ];
        let num_vertices = vertices.len() as u32;

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::bytes_of(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let instances = [vec3(0f32, 0., 0.), vec3(-0.5, 0., 0.), vec3(0.5, 0., 0.)];
        let num_instances = instances.len() as u32;

        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::bytes_of(&instances),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let render_pipeline = {
            let vertex_buffer_layouts = [
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of_val(&vertices[0]) as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0,
                    }],
                },
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of_val(&instances[0]) as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 1,
                    }],
                },
            ];

            let shader_module = device.create_shader_module(&wgpu::include_wgsl!("main.wgsl"));

            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: None,
                vertex: wgpu::VertexState {
                    module: &shader_module,
                    entry_point: "vs_main",
                    buffers: &vertex_buffer_layouts,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader_module,
                    entry_point: "fs_main",
                    targets: &[surface_format.into()],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            })
        };

        let proj_matrix = Mat4::orthographic_lh(-1f32, 1., -1., 1., 0., 1.);

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::bytes_of(&proj_matrix),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let bind_group = {
            let layout = render_pipeline.get_bind_group_layout(0);

            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &uniform_buffer,
                        offset: 0,
                        size: None,
                    }),
                }],
            })
        };

        Ok(Renderer {
            device,
            queue,
            surface,
            surface_format,
            surface_size,
            render_target,
            num_vertices,
            vertex_buffer,
            num_instances,
            instance_buffer,
            bind_group,
            render_pipeline,
        })
    }

    pub fn configure_surface(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        configure_surface(&self.surface, &self.device, self.surface_format, size);
        self.surface_size = wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        };
        self.render_target =
            create_render_target(&self.device, self.surface_size, self.surface_format);
    }

    pub fn render(&self) {
        let frame = self
            .surface
            .get_current_texture()
            .expect("Failed to get next surface texture");

        let render_target_view = self
            .render_target
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &render_target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
            render_pass.draw(0..self.num_vertices, 0..self.num_instances);
        }

        encoder.copy_texture_to_texture(
            self.render_target.as_image_copy(),
            frame.texture.as_image_copy(),
            self.surface_size,
        );

        self.queue.submit(Some(encoder.finish()));

        frame.present();
    }
}

fn configure_surface(
    surface: &wgpu::Surface,
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    size: winit::dpi::PhysicalSize<u32>,
) {
    surface.configure(
        device,
        &wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::COPY_DST,
            format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
        },
    );
}

fn create_render_target(
    device: &wgpu::Device,
    size: wgpu::Extent3d,
    format: wgpu::TextureFormat,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    })
}
