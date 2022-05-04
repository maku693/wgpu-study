use std::{f32::consts::PI, thread::sleep, time::Duration};

use anyhow::{Context, Result};
use glam::{vec3, EulerRot, Quat, Vec3};
use log::{debug, info};
use pollster::FutureExt;

mod entity;
mod renderer;

fn main() -> Result<()> {
    env_logger::init();

    let event_loop = winit::event_loop::EventLoop::new();

    let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);

    let window = winit::window::WindowBuilder::new()
        .with_title("antimodern")
        .with_inner_size(winit::dpi::LogicalSize::<u32> {
            width: 640,
            height: 360,
        })
        .build(&event_loop)
        .context("Failed to build window")?;

    let mut renderer = renderer::Renderer::new(&instance, &window).block_on()?;

    let mut scene = entity::Scene {
        camera: {
            let inner_size = window.inner_size();
            let aspect_ratio = inner_size.width as f32 / inner_size.height as f32;
            entity::Camera {
                transform: entity::Transform {
                    position: Vec3::ZERO,
                    rotation: Quat::IDENTITY,
                    ..Default::default()
                },
                fov: 60.,
                aspect_ratio,
                near: 0.1,
                far: 1000.,
            }
        },
        cube: entity::Cube {
            transform: entity::Transform {
                position: vec3(0., 0., 10.),
                rotation: Quat::from_axis_angle(Vec3::X, PI * -0.125),
                scale: Vec3::ONE,
            },
        },
        particle_system: entity::ParticleSystem {
            transform: entity::Transform {
                position: vec3(0., 0., 10.),
                rotation: Quat::from_axis_angle(Vec3::X, PI * -0.25),
                scale: Vec3::ONE * 1.5,
            },
            max_count: 10000,
            particle_size: 0.01,
            lifetime: 0,
            min_speed: 0.01,
            max_speed: 1.,
        },
    };

    info!("{:#?}", &scene);

    let cube_pipeline = renderer::cube::PipelineState::new(
        renderer.device(),
        renderer.surface_format(),
        renderer.depth_texture_format(),
        &scene,
    );

    let particle_pipeline = renderer::particles::PipelineState::new(
        renderer.device(),
        renderer.surface_format(),
        renderer.depth_texture_format(),
        &scene,
    );

    let billboard_pipeline = renderer::billboard::PipelineState::new(
        renderer.device(),
        renderer.surface_format(),
        renderer.depth_texture_format(),
        &scene,
    );

    std::thread::spawn(move || loop {
        instance.poll_all(true);
        sleep(Duration::from_millis(1));
    });

    let mut current_sample = 1;
    let mut cursor_locked = false;

    event_loop.run(move |e, _, control_flow| {
        use winit::{
            event::{
                DeviceEvent, ElementState, Event, KeyboardInput, MouseButton, MouseScrollDelta,
                VirtualKeyCode, WindowEvent,
            },
            event_loop::ControlFlow,
        };

        *control_flow = ControlFlow::Poll;

        debug!("{:#?}", e);

        match e {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::Resized(size) => {
                    renderer.resize(size);
                    scene.camera.aspect_ratio = size.width as f32 / size.height as f32;
                }
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    renderer.resize(*new_inner_size);
                    scene.camera.aspect_ratio =
                        new_inner_size.width as f32 / new_inner_size.height as f32;
                }
                WindowEvent::MouseInput {
                    state: ElementState::Released,
                    button: MouseButton::Left,
                    ..
                } => {
                    window.set_cursor_grab(true).unwrap();
                    window.set_cursor_visible(false);
                    cursor_locked = true;
                }
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            state: ElementState::Released,
                            virtual_keycode,
                            ..
                        },
                    ..
                } => match virtual_keycode {
                    Some(VirtualKeyCode::Escape) => {
                        window.set_cursor_grab(false).unwrap();
                        window.set_cursor_visible(true);
                        cursor_locked = false;
                    }
                    Some(VirtualKeyCode::Key1) => {
                        current_sample = 1;
                    }
                    Some(VirtualKeyCode::Key2) => {
                        current_sample = 2;
                    }
                    Some(VirtualKeyCode::Key3) => {
                        current_sample = 3;
                    }
                    _ => (),
                },
                _ => (),
            },
            Event::DeviceEvent { event, .. } => match event {
                DeviceEvent::MouseMotion { delta: (x, y) } => {
                    if !cursor_locked {
                        return;
                    };
                    let mut rotation = scene.camera.transform.rotation.to_euler(EulerRot::YXZ);
                    rotation.0 += x as f32 * 0.001;
                    rotation.1 = (rotation.1 + y as f32 * 0.001).clamp(PI * -0.5, PI * 0.5);
                    debug!("rotation: {:?}", rotation);
                    scene.camera.transform.rotation =
                        Quat::from_euler(glam::EulerRot::YXZ, rotation.0, rotation.1, rotation.2);
                }
                DeviceEvent::MouseWheel {
                    delta: MouseScrollDelta::PixelDelta(delta),
                } => {
                    if !cursor_locked {
                        return;
                    };
                    scene.camera.fov = (scene.camera.fov + delta.y as f32 * -0.1).clamp(30., 120.);
                    debug!("fov: {:?}", scene.camera.fov);
                }
                _ => (),
            },
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            Event::RedrawRequested(..) => {
                scene.cube.transform.rotation *= Quat::from_axis_angle(Vec3::Y, PI * 0.01);
                scene.particle_system.transform.rotation *=
                    Quat::from_axis_angle(Vec3::Y, PI * 0.001);

                cube_pipeline.update(&scene).block_on().unwrap();
                particle_pipeline.update(&scene).block_on().unwrap();
                billboard_pipeline.update(&scene).block_on().unwrap();

                match current_sample {
                    1 => renderer.render(&particle_pipeline),
                    2 => renderer.render(&cube_pipeline),
                    3 => renderer.render(&billboard_pipeline),
                    _ => (),
                };
            }
            _ => (),
        }
    });
}
