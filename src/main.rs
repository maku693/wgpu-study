use std::{
    sync::{Arc, RwLock},
    thread::sleep,
    time::Duration,
};

use anyhow::{Context, Result};
use glam::{vec3, Quat, Vec3};
use log::{debug, info};
use pollster::FutureExt;
use winit;

mod particles;
mod renderer;

fn main() -> Result<()> {
    env_logger::init();

    let event_loop = winit::event_loop::EventLoop::new();

    let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);

    let window = winit::window::WindowBuilder::new()
        .with_title("Hello, world!")
        .with_inner_size(winit::dpi::LogicalSize::<u32> {
            width: 640,
            height: 360,
        })
        .build(&event_loop)
        .context("Failed to build window")?;

    let renderer = Arc::new(RwLock::new(
        renderer::Renderer::new(&instance, &window).block_on()?,
    ));

    let scene = particles::entity::Scene {
        camera: {
            let inner_size = window.inner_size();
            let aspect_ratio = inner_size.width as f32 / inner_size.height as f32;
            particles::entity::Camera {
                position: vec3(0., 0., -10.),
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
            max_count: 1000,
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

    {
        std::thread::spawn(move || loop {
            instance.poll_all(true);
            sleep(Duration::from_millis(100));
        });
    }

    event_loop.run(move |e, _, control_flow| {
        use winit::{
            event::{Event, WindowEvent},
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
