use std::path::Path;

pub struct Image {
    inner: image::RgbaImage,
}

impl Image {
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self, String> {
        let dyn_image = image::open(path).map_err(|e| format!("{:?}", e))?;
        Ok(Self {
            inner: dyn_image.into_rgba8(),
        })
    }

    pub fn width(&self) -> u32 {
        self.inner.width()
    }

    pub fn height(&self) -> u32 {
        self.inner.height()
    }

    pub fn data(&self) -> &[u8] {
        &self.inner
    }
}
