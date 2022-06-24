pub trait BufferWriter {
    fn write_buffer(&self, buffer: &wgpu::Buffer, offset: u64, data: &[u8]);
}

impl BufferWriter for wgpu::Queue {
    fn write_buffer(&self, buffer: &wgpu::Buffer, offset: u64, data: &[u8]) {
        Self::write_buffer(self, buffer, offset, data);
    }
}
