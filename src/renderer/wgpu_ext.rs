pub struct Texture {
    wgpu_texture: wgpu::Texture,
    width: u32,
    height: u32,
    depth_or_array_layers: u32,
    mip_level_count: u32,
    sample_count: u32,
    dimension: wgpu::TextureDimension,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
}

impl Texture {
    pub fn wgpu_texture(&self) -> &wgpu::Texture {
        &self.wgpu_texture
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn depth_or_array_layers(&self) -> u32 {
        self.depth_or_array_layers
    }

    pub fn mip_level_count(&self) -> u32 {
        self.mip_level_count
    }

    pub fn sample_count(&self) -> u32 {
        self.sample_count
    }

    pub fn dimension(&self) -> wgpu::TextureDimension {
        self.dimension
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    pub fn usage(&self) -> wgpu::TextureUsages {
        self.usage
    }
}

impl AsRef<wgpu::Texture> for Texture {
    fn as_ref(&self) -> &wgpu::Texture {
        &self.wgpu_texture
    }
}

pub trait DeviceExt {
    fn create_texture_ext(&self, desc: &wgpu::TextureDescriptor) -> Texture;
}

impl DeviceExt for wgpu::Device {
    fn create_texture_ext(&self, desc: &wgpu::TextureDescriptor) -> Texture {
        Texture {
            wgpu_texture: self.create_texture(desc),
            width: desc.size.width,
            height: desc.size.height,
            depth_or_array_layers: desc.size.depth_or_array_layers,
            mip_level_count: desc.mip_level_count,
            sample_count: desc.sample_count,
            dimension: desc.dimension,
            format: desc.format,
            usage: desc.usage,
        }
    }
}
