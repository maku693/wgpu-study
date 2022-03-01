use anyhow::{Context, Result};

mod renderer;

fn main() -> Result<()> {
    env_logger::init();

    let event_loop = winit::event_loop::EventLoop::new();

    let monitor_size = event_loop
        .primary_monitor()
        .context("Failed to get primary monitor")?
        .size();

    let window = winit::window::WindowBuilder::new()
        .with_title("Hello, world!")
        .with_inner_size(winit::dpi::PhysicalSize {
            width: monitor_size.width / 2,
            height: monitor_size.height / 2,
        })
        .build(&event_loop)
        .context("Failed to build window")?;

    let mut renderer = renderer::Renderer::new(&window)?;

    event_loop.run(move |e, _, control_flow| {
        use winit::{
            event::{Event, WindowEvent},
            event_loop::ControlFlow,
        };

        *control_flow = ControlFlow::Poll;

        match e {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::Resized(size) => {
                    renderer.configure_surface(size);
                }
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    renderer.configure_surface(*new_inner_size);
                }
                _ => (),
            },
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            Event::RedrawRequested(..) => {
                renderer.render();
            }
            _ => (),
        }
    });
}
