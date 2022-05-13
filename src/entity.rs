use glam::{Quat, Vec3};

#[derive(Debug, Copy, Clone, Default)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Camera {
    pub transform: Transform,
    pub fov: f32,
    pub aspect_ratio: f32,
    pub near: f32,
    pub far: f32,
    pub exposure: f32,
}

#[derive(Debug, Copy, Clone, Default)]
pub struct ParticleSystem {
    pub transform: Transform,
    pub max_count: u32,
    pub particle_size: f32,
    pub lifetime: u32,
    pub min_speed: f32,
    pub max_speed: f32,
}

#[derive(Debug, Copy, Clone, Default)]
pub struct BloomEffect {
    pub intensity: u32,
    pub threshold: u32,
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Scene {
    pub camera: Camera,
    pub particle_system: ParticleSystem,
    pub bloom_effect: BloomEffect,
}
