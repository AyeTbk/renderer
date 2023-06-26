use crate::{arena::Handle, image::Image, Color};

pub struct Material {
    pub base_color: Color,
    pub base_color_image: Option<Handle<Image>>,
    pub billboard_mode: BillboardMode,
    pub unlit: bool,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            base_color: Color::WHITE,
            base_color_image: None,
            billboard_mode: BillboardMode::Off,
            unlit: false,
        }
    }
}

pub enum BillboardMode {
    Off,
    On,
    FixedSize,
}
