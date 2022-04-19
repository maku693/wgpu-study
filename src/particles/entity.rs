use glam::{Quat, Vec3};

#[derive(Debug, Copy, Clone, Default)]
pub struct Scene {
    pub camera: Camera,
    pub particle_system: ParticleSystem,
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

#[derive(Debug, Copy, Clone, Default)]
pub struct ParticleSystem {
    pub position: Vec3,
    pub rotation: Quat,
    pub num_particles: u32,
}
