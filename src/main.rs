use std::{
    f32::consts::PI,
    sync::{Arc, RwLock},
    thread::sleep,
    time::Duration,
};

use anyhow::{Context, Result};
use glam::{vec3, EulerRot, Quat, Vec3};
use log::{debug, info};
use pollster::FutureExt;

mod particles;
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

    window.set_cursor_grab(true)?;
    window.set_cursor_visible(false);

    let renderer = Arc::new(RwLock::new(
        renderer::Renderer::new(&instance, &window).block_on()?,
    ));

    let mut scene = particles::entity::Scene {
        camera: {
            let inner_size = window.inner_size();
            let aspect_ratio = inner_size.width as f32 / inner_size.height as f32;
            particles::entity::Camera {
                position: vec3(0., 0., -100.),
                rotation: Quat::IDENTITY,
                fov: 60.,
                aspect_ratio,
                near: 0.,
                far: 1000.,
            }
        },
        particle_system: particles::entity::ParticleSystem {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            max_count: 1000000,
            lifetime: 0,
            min_speed: 0.1,
            max_speed: 1.,
        },
    };

    info!("{:#?}", &scene);

    let pipeline = {
        let renderer = renderer.read().unwrap();
        particles::pipeline::PipelineState::new(
            renderer.device(),
            renderer.surface_format(),
            &scene,
        )
    };

    std::thread::spawn(move || loop {
        instance.poll_all(true);
        sleep(Duration::from_millis(1));
    });

    event_loop.run(move |e, _, control_flow| {
        use winit::{
            event::{DeviceEvent, Event, MouseScrollDelta, WindowEvent},
            event_loop::ControlFlow,
        };

        *control_flow = ControlFlow::Poll;

        debug!("{:#?}", e);

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
            Event::DeviceEvent { event, .. } => match event {
                DeviceEvent::MouseMotion { delta: (x, y) } => {
                    let mut rotation = scene.camera.rotation.to_euler(EulerRot::YXZ);
                    rotation.0 += x as f32 * 0.001;
                    rotation.1 = (rotation.1 + y as f32 * 0.001).clamp(PI * -0.5, PI * 0.5);
                    debug!("rotation: {:?}", rotation);
                    scene.camera.rotation =
                        Quat::from_euler(glam::EulerRot::YXZ, rotation.0, rotation.1, rotation.2);
                }
                DeviceEvent::MouseWheel {
                    delta: MouseScrollDelta::PixelDelta(delta),
                } => {
                    scene.camera.fov = (scene.camera.fov + delta.y as f32 * -0.1).clamp(30., 90.);
                    debug!("fov: {:?}", scene.camera.fov);
                }
                _ => (),
            },
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            Event::RedrawRequested(..) => {
                pipeline.update(&scene).block_on().unwrap();
                renderer.read().unwrap().render(&pipeline);
            }
            _ => (),
        }
    });
}
