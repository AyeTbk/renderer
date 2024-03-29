use crate::Color;

#[derive(Clone)]
pub struct Light {
    pub color: Color,
    pub kind: LightKind,
}

impl Light {
    pub fn directional() -> Self {
        Self {
            kind: LightKind::Directional,
            ..Default::default()
        }
    }

    pub fn point(radius: f32) -> Self {
        Self {
            kind: LightKind::Point { radius },
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

#[derive(Clone)]
pub enum LightKind {
    Directional,
    Point { radius: f32 },
}

impl LightKind {
    pub fn id(&self) -> u32 {
        match &self {
            LightKind::Directional { .. } => 0,
            LightKind::Point { .. } => 1,
        }
    }
}
