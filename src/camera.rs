use glam::Mat4;

#[derive(Debug)]
pub struct Camera {
    projection: Mat4,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            projection: Mat4::perspective_lh(std::f32::consts::FRAC_PI_2, 1.667, 0.01, 1000.0),
        }
    }
}
