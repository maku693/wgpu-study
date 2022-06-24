use crate::component;

#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct Camera {
    pub transform: component::Transform,
    pub camera: component::Camera,
}

#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct Particle {
    pub transform: component::Transform,
    pub particle: component::Particle,
}

#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct PostProcessing {
    pub bloom: component::Bloom,
}

#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct Scene {
    pub camera: Camera,
    pub particle: Particle,
    pub post_processing: PostProcessing,
}
