pub use asset_image::Image;

use crate::asset_server::{Asset, Loadable, Loader};

impl Loadable for Image {
    fn new_placeholder() -> Self {
        Self::new_dummy()
    }

    fn new_loader() -> Box<dyn Loader> {
        Box::new(ImageLoader)
    }
}

pub struct ImageLoader;

impl Loader for ImageLoader {
    fn load_from_path(&self, path: &str) -> Result<Box<dyn Asset>, String> {
        let mut image = Image::load_from_path(path)?;
        let _ = image.make_mips();
        Ok(Box::new(image))
    }
}
