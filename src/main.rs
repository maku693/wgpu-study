use wgpu::util::DeviceExt;

#[tokio::main]
async fn main() {
    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_title("Hello, world!")
        .with_resizable(false)
        .with_inner_size(winit::dpi::LogicalSize {
            width: 1280,
            height: 720,
        })
        .build(&event_loop)
        .expect("Failed to build window");

    let renderer = Renderer::new(&window).await;
    renderer.configure_surface(window.inner_size());

    tokio::task::block_in_place(move || {
        event_loop.run(move |e, _, control_flow| {
            use winit::{
                event::{Event, WindowEvent},
                event_loop::ControlFlow,
            };

            *control_flow = ControlFlow::Wait;

            match e {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    *control_flow = ControlFlow::Exit;
                }
                Event::WindowEvent {
                    event: WindowEvent::Resized(size),
                    ..
                } => {
                    renderer.configure_surface(size);
                }
                Event::MainEventsCleared => {
                    renderer.render();
                }
                _ => (),
            }
        });
    })
}

struct Renderer {
    surface: wgpu::Surface,
    surface_format: wgpu::TextureFormat,
    device: wgpu::Device,
    queue: wgpu::Queue,
    num_vertices: u32,
    vertex_buffer: wgpu::Buffer,
    render_pipeline: wgpu::RenderPipeline,
}

impl Renderer {
    async fn new(window: &winit::window::Window) -> Renderer {
        let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);
        let surface = unsafe { instance.create_surface(&window) };

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("No adapter found");

        let surface_format = surface
            .get_preferred_format(&adapter)
            .expect("Surface is incompatible with adapter");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .expect("No device found");

        let (num_vertices, vertex_buffer, render_pipeline) = {
            let vertices: [f32; 6] = [-1., -1., 0., 1., 1., -1.];
            let elements_per_vertex = 2;
            let num_vertices = (vertices.len() / elements_per_vertex) as u32;

            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

            let vertex_buffer_layouts = [wgpu::VertexBufferLayout {
                array_stride: (std::mem::size_of::<f32>() * elements_per_vertex)
                    as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                }],
            }];

            let shader_module = device.create_shader_module(&wgpu::include_wgsl!("main.wgsl"));

            let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
            });

            (num_vertices, vertex_buffer, render_pipeline)
        };

        Renderer {
            device,
            queue,
            surface,
            surface_format,
            num_vertices,
            vertex_buffer,
            render_pipeline,
        }
    }

    fn configure_surface(&self, size: winit::dpi::PhysicalSize<u32>) {
        self.surface.configure(
            &self.device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: self.surface_format,
                width: size.width,
                height: size.height,
                present_mode: wgpu::PresentMode::Fifo,
            },
        );
    }

    fn render(&self) {
        let frame = self
            .surface
            .get_current_texture()
            .expect("Failed to get next surface texture");
        let frame_view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &frame_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.draw(0..self.num_vertices, 0..1);
        }

        self.queue.submit(Some(encoder.finish()));

        frame.present();
    }
}
