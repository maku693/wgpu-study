use std::time::{Duration, Instant};

use anyhow::Result;
use log::debug;
use smol::{block_on, LocalExecutor};
use winit::{
    event::{DeviceEvent, ElementState, Event, KeyboardInput, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
};

use crate::app::App;

mod app;
mod entity;
mod renderer;

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

    let mut last_render_inst = Instant::now();

    let mut app = block_on(App::new(window))?;

    event_loop.run(move |e, _, control_flow| {
        debug!("{:#?}", e);

        match e {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::Resized(size) => app.resize(size),
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    app.resize(*new_inner_size)
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
                while executer.try_tick() {}

                let target_frame_interval = Duration::from_secs_f64(1.0 / 60.0);
                let elapsed_from_last_draw = last_render_inst.elapsed();
                if target_frame_interval > elapsed_from_last_draw {
                    let wait = target_frame_interval - elapsed_from_last_draw;
                    *control_flow = ControlFlow::WaitUntil(Instant::now() + wait);
                    return;
                }

                last_render_inst = Instant::now();

                executer.spawn(app.render()).detach();
            }
            _ => (),
        }
    });
}
