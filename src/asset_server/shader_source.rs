use std::path::Path;

pub struct ShaderSource {
    src: String,
}

impl ShaderSource {
    pub fn new(src: String) -> Self {
        Self { src }
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, String> {
        let src = std::fs::read_to_string(path).map_err(|e| format!("{:?}", e))?;
        Ok(Self::new(src))
    }

    pub fn source(&self) -> &str {
        &self.src
    }

    pub fn validate(&self) -> Result<(), String> {
        match naga::front::wgsl::parse_str(self.source()) {
            Err(parse_error) => {
                parse_error.emit_to_stderr(self.source());
                return Err(parse_error.emit_to_string(self.source()));
            }
            Ok(module) => {
                use naga::valid::*;
                let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
                if let Err(validation_error) = validator.validate(&module) {
                    validation_error.emit_to_stderr(self.source());
                    return Err(validation_error.emit_to_string(self.source()));
                }
            }
        }
        Ok(())
    }
}
