pub use asset_shader_source::ShaderSource;

use crate::asset_server::{Asset, Loadable, Loader};

impl Loadable for ShaderSource {
    fn new_placeholder() -> Self {
        Self::new(String::new())
    }

    fn new_loader(options: &str) -> Box<dyn Loader> {
        Box::new(ShaderSourceLoader::new(options))
    }
}

pub struct ShaderSourceLoader {
    defines: Vec<String>,
}

impl ShaderSourceLoader {
    pub fn new(options: &str) -> Self {
        let defines = options
            .split(',')
            .map(|d| d.trim().to_string())
            .collect::<Vec<_>>();
        Self { defines }
    }
}

impl Loader for ShaderSourceLoader {
    fn load_from_path(&mut self, path: &str) -> Result<Box<dyn Asset>, String> {
        let shader_source = ShaderSource::load_from_path(path, std::mem::take(&mut self.defines))?;
        shader_source.validate()?;
        Ok(Box::new(shader_source))
    }

    fn only_sync(&self) -> bool {
        true
    }
}
