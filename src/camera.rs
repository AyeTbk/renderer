use glam::Mat4;

#[derive(Debug, Clone)]
pub struct Camera {
    pub vfov: f32,
    pub aspect_ratio: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            vfov: 1.3,
            aspect_ratio: 1.667,
            near: 0.05,
            far: 100.0,
        }
    }
}

impl Camera {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective_lh(self.vfov, self.aspect_ratio, self.near, self.far)
    }
}
