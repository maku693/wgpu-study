use glam::{Quat, Vec3};

#[derive(Debug, Copy, Clone, Default)]
pub struct Cube {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}
