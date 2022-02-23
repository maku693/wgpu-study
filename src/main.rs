use anyhow::{Context, Result};

mod renderer;

fn main() -> Result<()> {
    env_logger::init();

    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_title("Hello, world!")
        .with_inner_size(winit::dpi::LogicalSize {
            width: 640,
            height: 360,
        })
        .build(&event_loop)
        .context("Failed to build window")?;

    let renderer = renderer::Renderer::new(&window)?;
    renderer.configure_surface(window.inner_size());

    event_loop.run(move |e, _, control_flow| {
        use winit::{
            event::{Event, WindowEvent},
            event_loop::ControlFlow,
        };

        *control_flow = ControlFlow::Poll;

        match e {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::Resized(size) => renderer.configure_surface(size),
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
