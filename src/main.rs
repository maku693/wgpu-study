use std::{error, mem};

use tokio::task;
use wgpu::util::DeviceExt;
use winit::{dpi, event, event_loop, window};

#[tokio::main]
async fn main() -> Result<(), Box<dyn error::Error>> {
    let event_loop = event_loop::EventLoop::new();
    let window = window::WindowBuilder::new()
        .with_title("Hello, world!")
        .with_resizable(false)
        .with_inner_size(dpi::LogicalSize {
            width: 1280,
            height: 720,
        })
        .build(&event_loop)?;

    let inner_size = window.inner_size();

    let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);
    let surface = unsafe { instance.create_surface(&window) };

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })
        .await
        .ok_or("No adapter found")?;

    let surface_format = surface
        .get_preferred_format(&adapter)
        .ok_or("Surface is incompatible with adapter")?;

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

    let shader_module = device.create_shader_module(&wgpu::include_wgsl!("main.wgsl"));

    let vertices: [f32; 6] = [-1., -1., 0., 1., 1., -1.];
    let elements_per_vertex = 2;
    let num_vertices = (vertices.len() / elements_per_vertex) as u32;
    let vertex_size = mem::size_of::<f32>() * elements_per_vertex;

    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let vertex_buffer_layouts = [wgpu::VertexBufferLayout {
        array_stride: vertex_size as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x2,
            offset: 0,
            shader_location: 0,
        }],
    }];

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

    surface.configure(
        &device,
        &wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: inner_size.width,
            height: inner_size.height,
            present_mode: wgpu::PresentMode::Mailbox,
        },
    );

    task::block_in_place(move || {
        event_loop.run(move |event, _, control_flow| {
            *control_flow = event_loop::ControlFlow::Wait;

            match event {
                event::Event::WindowEvent {
                    event: event::WindowEvent::CloseRequested,
                    ..
                } => *control_flow = event_loop::ControlFlow::Exit,
                event::Event::WindowEvent {
                    event: event::WindowEvent::Resized(size),
                    ..
                } => {
                    surface.configure(
                        &device,
                        &wgpu::SurfaceConfiguration {
                            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                            format: surface_format,
                            width: size.width,
                            height: size.height,
                            present_mode: wgpu::PresentMode::Mailbox,
                        },
                    );
                }
                event::Event::MainEventsCleared => {
                    let frame = surface
                        .get_current_texture()
                        .expect("Failed to get next surface texture");
                    let frame_view = frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());

                    let mut encoder = device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

                    {
                        let mut render_pass =
                            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
                        render_pass.set_pipeline(&render_pipeline);
                        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                        render_pass.draw(0..num_vertices, 0..1);
                    }

                    queue.submit(Some(encoder.finish()));

                    frame.present();
                }
                _ => (),
            }
        });
    })
}
