pub struct Surface {
    pub wgpu_surface: wgpu::Surface,
    pub texture_format: wgpu::TextureFormat,
}

impl Surface {
    pub fn new(wgpu_surface: wgpu::Surface, texture_format: wgpu::TextureFormat) -> Self {
        Self {
            wgpu_surface,
            texture_format,
        }
    }

    pub fn configure(&self, device: &wgpu::Device, width: u32, height: u32) {
        self.wgpu_surface.configure(
            device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: self.texture_format,
                width,
                height,
                present_mode: wgpu::PresentMode::Fifo,
            },
        );
    }
}
