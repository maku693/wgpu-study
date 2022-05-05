use std::{f32::consts::PI, future::Future};

use anyhow::{Ok, Result};
use glam::{vec3, EulerRot, Quat, Vec3};
use log::{debug, info};
use winit::{
    dpi::PhysicalPosition,
    event::{MouseScrollDelta, VirtualKeyCode},
    window::Window,
};

use crate::{
    entity::{Camera, ParticleSystem, Scene, Transform},
    renderer::Renderer,
};

pub struct App {
    window: Window,
    scene: Scene,
    renderer: Renderer,
    cursor_locked: bool,
}

impl App {
    pub async fn new(window: Window) -> Result<Self> {
        let scene = Scene {
            camera: {
                let inner_size = window.inner_size();
                let aspect_ratio = inner_size.width as f32 / inner_size.height as f32;
                Camera {
                    transform: Transform {
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
            particle_system: ParticleSystem {
                transform: Transform {
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

        let renderer = Renderer::new(&window, &scene).await?;

        Ok(Self {
            window,
            scene,
            renderer,
            cursor_locked: false,
        })
    }

    pub fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        self.renderer.recreate_render_attachments(size);
    }

    pub fn on_mouse_up(&mut self) {
        self.window.set_cursor_grab(true).unwrap();
        self.window.set_cursor_visible(false);
        self.cursor_locked = true;
    }

    pub fn on_key_up(&mut self, keycode: VirtualKeyCode) {
        match keycode {
            VirtualKeyCode::Escape => {
                self.window.set_cursor_grab(false).unwrap();
                self.window.set_cursor_visible(true);
                self.cursor_locked = false;
            }
            _ => (),
        }
    }

    pub fn on_mouse_move(&mut self, (x, y): (f64, f64)) {
        if !self.cursor_locked {
            return;
        };

        let mut rotation = self.scene.camera.transform.rotation.to_euler(EulerRot::YXZ);
        rotation.0 += x as f32 * 0.001;
        rotation.1 = (rotation.1 + y as f32 * 0.001).clamp(PI * -0.5, PI * 0.5);
        debug!("rotation: {:?}", rotation);

        self.scene.camera.transform.rotation =
            Quat::from_euler(glam::EulerRot::YXZ, rotation.0, rotation.1, rotation.2);
    }

    pub fn on_mouse_scroll(&mut self, delta: winit::event::MouseScrollDelta) {
        if !self.cursor_locked {
            return;
        };
        let y = match delta {
            MouseScrollDelta::PixelDelta(PhysicalPosition { y, .. }) => y as f32,
            MouseScrollDelta::LineDelta(_, y) => y * 60.0,
        };

        self.scene.camera.fov = (self.scene.camera.fov + y * -0.1).clamp(30., 120.);
    }

    pub fn render(&mut self) -> impl Future<Output = ()> {
        self.scene.particle_system.transform.rotation *= Quat::from_axis_angle(Vec3::Y, PI * 0.01);
        self.renderer.render(&self.scene)
    }
}
