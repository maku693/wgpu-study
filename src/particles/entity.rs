use glam::{Quat, Vec3};

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
