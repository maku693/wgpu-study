use std::{
    sync::{Arc, RwLock},
    thread::sleep,
    time::Duration,
};

use anyhow::{Context, Result};
use pollster::FutureExt;
use winit;

mod renderer;

fn main() -> Result<()> {
    env_logger::init();

    let event_loop = winit::event_loop::EventLoop::new();

    let window = winit::window::WindowBuilder::new()
        .with_title("Hello, world!")
        .with_inner_size(winit::dpi::LogicalSize::<u32> {
            width: 640,
            height: 360,
        })
        .build(&event_loop)
        .context("Failed to build window")?;

    let renderer = Arc::new(RwLock::new(renderer::Renderer::new(&window).block_on()?));

    {
        let renderer = renderer.clone();
        std::thread::spawn(move || loop {
            renderer.read().unwrap().poll_device();
            sleep(Duration::from_millis(100));
        });
    }

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
                    renderer.write().unwrap().resize_surface(size);
                }
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    renderer.write().unwrap().resize_surface(*new_inner_size);
                }
                _ => (),
            },
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            Event::RedrawRequested(..) => {
                renderer.read().unwrap().render();
            }
            _ => (),
        }
    });
}
