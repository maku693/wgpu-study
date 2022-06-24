use glam::{Quat, Vec3};

#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct Camera {
    pub fov: f32,
    pub aspect_ratio: f32,
    pub near: f32,
    pub far: f32,
    pub exposure: f32,
}

#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct Particle {
    pub max_count: u32,
    pub particle_size: f32,
    pub color_range: (Vec3, Vec3),
    pub position_range: (Vec3, Vec3),
}

#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct Bloom {
    pub intensity: f32,
    pub threshold: f32,
}
