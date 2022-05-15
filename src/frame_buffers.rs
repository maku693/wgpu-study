use std::num::NonZeroU32;

pub struct FrameBuffers {
    pub color_texture: wgpu::Texture,
    pub color_texture_view: wgpu::TextureView,
    pub depth_texture: wgpu::Texture,
    pub depth_texture_view: wgpu::TextureView,
    pub bright_texture: wgpu::Texture,
    pub bright_texture_view: wgpu::TextureView,
    pub bloom_texture: wgpu::Texture,
    pub bloom_texture_view: wgpu::TextureView,
}

impl FrameBuffers {
    pub const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;
    pub const BLOOM_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let color_texture = Self::create_color_texture(device, width, height);
        let color_texture_view = Self::create_color_texture_view(&color_texture);

        let depth_texture = Self::create_depth_texture(device, width, height);
        let depth_texture_view = Self::create_depth_texture_view(&depth_texture);

        let bright_texture = Self::create_bright_texture(device, width, height);
        let bright_texture_view = Self::create_bright_texture_view(&bright_texture);

        let bloom_texture = Self::create_bloom_texture(device, width, height);
        let bloom_texture_view = Self::create_bloom_texture_view(&bloom_texture, 0);

        Self {
            color_texture,
            color_texture_view,
            depth_texture,
            depth_texture_view,
            bright_texture,
            bright_texture_view,
            bloom_texture,
            bloom_texture_view,
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
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
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

    fn create_bright_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Bloom Bright Texture"),
            size: wgpu::Extent3d {
                width: width / 4,
                height: height / 4,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::BLOOM_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        })
    }

    fn create_bright_texture_view(texture: &wgpu::Texture) -> wgpu::TextureView {
        texture.create_view(&wgpu::TextureViewDescriptor {
            ..Default::default()
        })
    }

    fn create_bloom_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Bloom Texture"),
            size: wgpu::Extent3d {
                width: width / 4,
                height: height / 4,
                depth_or_array_layers: 4,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::BLOOM_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        })
    }

    fn create_bloom_texture_view(texture: &wgpu::Texture, base_layer: u32) -> wgpu::TextureView {
        texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2),
            base_array_layer: base_layer,
            array_layer_count: NonZeroU32::new(1),
            ..Default::default()
        })
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.color_texture = Self::create_color_texture(device, width, height);
        self.color_texture_view = Self::create_color_texture_view(&self.color_texture);
        self.depth_texture = Self::create_depth_texture(device, width, height);
        self.depth_texture_view = Self::create_depth_texture_view(&self.depth_texture);
        self.bright_texture = Self::create_bright_texture(device, width, height);
        self.bright_texture_view = Self::create_bright_texture_view(&self.bright_texture);
        self.bloom_texture = Self::create_bloom_texture(device, width, height);
        self.bloom_texture_view = Self::create_bloom_texture_view(&self.bloom_texture, 0);
    }
}
