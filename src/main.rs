mod renderer;

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

    let renderer = renderer::Renderer::new(&window).await;
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
