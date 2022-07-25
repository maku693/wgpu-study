pub trait HasSize {
    fn size(&self) -> Size;
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl From<winit::dpi::PhysicalSize<u32>> for Size {
    fn from(size: winit::dpi::PhysicalSize<u32>) -> Self {
        Self {
            width: size.width,
            height: size.height,
        }
    }
}

impl HasSize for winit::window::Window {
    fn size(&self) -> Size {
        self.inner_size().into()
    }
}

pub trait Window: HasSize + raw_window_handle::HasRawWindowHandle {}

impl Window for winit::window::Window {}
