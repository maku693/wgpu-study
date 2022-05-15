pub struct Samplers {
    pub bilinear: wgpu::Sampler,
}

impl Samplers {
    pub fn new(device: &wgpu::Device) -> Self {
        let bilinear = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Bilinear Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self { bilinear }
    }
}
