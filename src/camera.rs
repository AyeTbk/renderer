use glam::{Mat4, Vec3};

#[derive(Debug)]
pub struct Camera {
    pub projection: Mat4,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            projection: Mat4::perspective_lh(1.3, 1.667, 0.01, 1000.0)
                * Mat4::from_translation(-Vec3::new(0.0, 0.0, 0.0)),
        }
    }
}
