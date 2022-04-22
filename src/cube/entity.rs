use glam::{Quat, Vec3};

#[derive(Debug, Copy, Clone, Default)]
pub struct Scene {
    pub camera: Camera,
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
    pub scale: Vec3,
    pub max_count: u32,
    pub lifetime: u32,
    pub min_speed: f32,
    pub max_speed: f32,
}
