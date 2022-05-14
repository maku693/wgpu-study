pub struct FrameBuffers {
    pub color_texture: wgpu::Texture,
    pub color_texture_view: wgpu::TextureView,
    pub depth_texture: wgpu::Texture,
    pub depth_texture_view: wgpu::TextureView,
}

impl FrameBuffers {
    pub const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;

    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let color_texture = Self::create_color_texture(device, width, height);
        let color_texture_view = Self::create_color_texture_view(&color_texture);

        let depth_texture = Self::create_depth_texture(device, width, height);
        let depth_texture_view = Self::create_depth_texture_view(&depth_texture);

        Self {
            color_texture,
            color_texture_view,
            depth_texture,
            depth_texture_view,
        }
    }

    fn create_color_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Offscreen Color Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::COLOR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        })
    }

    fn create_color_texture_view(texture: &wgpu::Texture) -> wgpu::TextureView {
        texture.create_view(&wgpu::TextureViewDescriptor {
            ..Default::default()
        })
    }

    fn create_depth_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        })
    }

    fn create_depth_texture_view(texture: &wgpu::Texture) -> wgpu::TextureView {
        texture.create_view(&wgpu::TextureViewDescriptor {
            aspect: wgpu::TextureAspect::DepthOnly,
            ..Default::default()
        })
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.color_texture = Self::create_color_texture(device, width, height);
        self.color_texture_view = Self::create_color_texture_view(&self.color_texture);
        self.depth_texture = Self::create_depth_texture(device, width, height);
        self.depth_texture_view = Self::create_depth_texture_view(&self.depth_texture);
    }
}
