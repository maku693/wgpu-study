use anyhow::{Context, Result};

use crate::renderer;

pub struct App {
    event_loop: winit::event_loop::EventLoop<()>,
    window: winit::window::Window,
    renderer: renderer::Renderer,
}

impl App {
    pub fn new() -> Result<Self> {
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

        Ok(Self {
            event_loop,
            window,
            renderer,
        })
    }

    pub fn run(self) {
        self.event_loop.run(move |e, _, control_flow| {
            use winit::{
                event::{Event, WindowEvent},
                event_loop::ControlFlow,
            };

            *control_flow = ControlFlow::Poll;

            match e {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                    WindowEvent::Resized(size) => self.renderer.configure_surface(size),
                    _ => (),
                },
                Event::MainEventsCleared => {
                    self.window.request_redraw();
                }
                Event::RedrawRequested(..) => {
                    self.renderer.render();
                }
                _ => (),
            }
        })
    }
}
