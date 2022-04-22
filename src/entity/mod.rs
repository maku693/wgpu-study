use glam::{Quat, Vec3};

use crate::{cube, particles};

#[derive(Debug, Copy, Clone, Default)]
pub struct Scene {
    pub camera: Camera,
    pub cube: cube::entity::Cube,
    pub particle_system: particles::entity::ParticleSystem,
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Camera {
    pub position: Vec3,
    pub rotation: Quat,
    pub fov: f32,
    pub aspect_ratio: f32,
    pub near: f32,
    pub far: f32,
}
