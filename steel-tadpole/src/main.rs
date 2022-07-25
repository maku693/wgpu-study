use std::time::{Duration, Instant};

use anyhow::Result;
use log::debug;
use pollster::FutureExt as _;
use winit::{
    dpi::LogicalSize,
    event::{DeviceEvent, ElementState, Event, KeyboardInput, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

mod app;
mod component;
mod entity;
mod renderer;
mod window;

use app::App;

fn main() -> Result<()> {
    env_logger::init();

    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("wgpu example")
        .with_inner_size(LogicalSize::<u32> {
            width: 640,
            height: 360,
        })
        .build(&event_loop)?;

    let mut last_render_inst = Instant::now();

    let mut app = App::new(window).block_on()?;

    event_loop.run(move |e, _, control_flow| {
        debug!("{:#?}", e);

        match e {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::Resized(size) => app.on_resize(size),
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    app.on_resize(*new_inner_size)
                }
                WindowEvent::MouseInput {
                    state: ElementState::Released,
                    button: MouseButton::Left,
                    ..
                } => app.on_mouse_up(),
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            state: ElementState::Released,
                            virtual_keycode: Some(keycode),
                            ..
                        },
                    ..
                } => app.on_key_up(keycode),
                _ => (),
            },
            Event::DeviceEvent { event, .. } => match event {
                DeviceEvent::MouseMotion { delta } => app.on_mouse_move(delta),
                DeviceEvent::MouseWheel { delta } => app.on_mouse_scroll(delta),
                _ => (),
            },
            Event::MainEventsCleared => {
                let target_frame_interval = Duration::from_secs_f64(1.0 / 60.0);
                let elapsed_from_last_draw = last_render_inst.elapsed();
                if target_frame_interval > elapsed_from_last_draw {
                    let wait = target_frame_interval - elapsed_from_last_draw;
                    *control_flow = ControlFlow::WaitUntil(Instant::now() + wait);
                    return;
                }

                app.render();

                last_render_inst = Instant::now();
            }
            _ => (),
        }
    });
}
