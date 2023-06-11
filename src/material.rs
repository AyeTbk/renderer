use crate::Color;

pub struct Material {
    pub base_color: Color,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            base_color: Color::WHITE,
        }
    }
}
