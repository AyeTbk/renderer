use crate::Color;

#[derive(Clone)]
pub struct Light {
    pub color: Color,
    pub kind: LightKind,
}

#[derive(Clone)]
pub enum LightKind {
    Directional,
    Point { radius: f32 },
}

impl Light {
    pub fn directional() -> Self {
        Self {
            kind: LightKind::Directional,
            ..Default::default()
        }
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn radius(&self) -> Option<f32> {
        match &self.kind {
            LightKind::Point { radius } => Some(*radius),
            _ => None,
        }
    }
}

impl Default for Light {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
            kind: LightKind::Point { radius: 1.0 },
        }
    }
}
