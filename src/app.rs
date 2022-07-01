use std::{
    f32::consts::{PI, TAU},
    time::Instant,
};

use anyhow::{Ok, Result};
use glam::{vec3, EulerRot, Quat, Vec3};
use log::{debug, info};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{MouseScrollDelta, VirtualKeyCode},
    window::Window,
};

use crate::{
    component,
    entity::{Camera, Particle, PostProcessing, Scene},
    renderer::Renderer,
};

pub struct App {
    window: Window,
    scene: Scene,
    renderer: Renderer,
    new_at: Instant,
    cursor_locked: bool,
}

impl App {
    pub async fn new(window: Window) -> Result<Self> {
        let new_at = Instant::now();

        let scene = Scene {
            camera: {
                let inner_size = window.inner_size();
                let aspect_ratio = inner_size.width as f32 / inner_size.height as f32;
                Camera {
                    transform: component::Transform {
                        position: Vec3::ZERO,
                        rotation: Quat::IDENTITY,
                        ..Default::default()
                    },
                    camera: component::Camera {
                        fov: 60.,
                        aspect_ratio,
                        near: 0.1,
                        far: 1000.,
                        exposure: 1.0,
                    },
                }
            },
            particle: Particle {
                transform: component::Transform {
                    position: vec3(0., 0., 10.),
                    rotation: Quat::from_axis_angle(Vec3::X, PI * -0.25),
                    scale: Vec3::ONE * 1.5,
                },
                particle: component::Particle {
                    max_count: 1000,
                    particle_size: 0.01,
                    position_range: (Vec3::ONE * -0.5, Vec3::ONE * 0.5),
                    color_range: (Vec3::ONE * 5.0, Vec3::ONE * 50.0),
                },
            },
            post_processing: PostProcessing {
                bloom: component::Bloom {
                    intensity: 1.0,
                    threshold: 1.0,
                },
            },
        };
        info!("{:#?}", &scene);

        let renderer = Renderer::new(&window, &scene).await?;

        Ok(Self {
            window,
            scene,
            renderer,
            new_at,
            cursor_locked: false,
        })
    }

    pub fn on_resize(&mut self, size: PhysicalSize<u32>) {
        // HACK: Ignore incorrect initial window resize event on windows
        let current_inner_size = self.window.inner_size();
        if size.width > current_inner_size.width || size.height > current_inner_size.height {
            return;
        }

        self.scene.camera.camera.aspect_ratio = size.width as f32 / size.height as f32;

        self.renderer.resize(size.into());
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
            VirtualKeyCode::K => {
                self.scene.camera.camera.exposure += 0.1;
                info!(
                    "Camera exposure increased: {}",
                    self.scene.camera.camera.exposure
                );
            }
            VirtualKeyCode::J => {
                self.scene.camera.camera.exposure -= 0.1;
                info!(
                    "Camera exposure decreased: {}",
                    self.scene.camera.camera.exposure
                );
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
        self.scene.camera.camera.fov = (self.scene.camera.camera.fov + y * -0.1).clamp(30., 120.);
    }

    pub fn render(&mut self) {
        let now = Instant::now().duration_since(self.new_at).as_millis() as f32 * 0.001;

        self.scene.particle.transform.rotation *= Quat::from_axis_angle(Vec3::Y, PI * 0.001);

        let scale = ((TAU * now * 0.01).cos() + 1.0) * 0.5;
        let scale = scale * 8.0 + 2.0;
        self.scene.particle.transform.scale = Vec3::ONE * scale;

        self.renderer.render(&self.scene);
    }
}
