use std::path::Path;

pub struct Image {
    inner: image::RgbaImage,
    mips: Option<Mips>,
}

impl Image {
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self, String> {
        let dyn_image = image::open(path).map_err(|e| format!("{:?}", e))?;
        Ok(Self {
            inner: dyn_image.into_rgba8(),
            mips: None,
        })
    }

    pub fn make_mips(&mut self, max_level: Option<u32>) -> Result<(), String> {
        assert!(self.width().is_power_of_two());
        assert!(self.height() == self.width());

        let level0_size = self.width();
        let levels = (level0_size.ilog2() + 1).min(max_level.unwrap_or(u32::MAX));
        let mut data: Vec<u8> = Vec::new();

        // level 0:
        data.extend(self.data());

        // level 1..n:
        for level in 1..levels {
            let level_size = level0_size >> level;
            let mip = image::imageops::resize(
                &self.inner,
                level_size,
                level_size,
                image::imageops::FilterType::Triangle,
            );
            data.extend(mip.iter());
        }

        self.mips = Some(Mips {
            levels: levels,
            data,
        });
        Ok(())
    }

    pub fn width(&self) -> u32 {
        self.inner.width()
    }

    pub fn height(&self) -> u32 {
        self.inner.height()
    }

    pub fn data(&self) -> &[u8] {
        if let Some(mips) = &self.mips {
            &mips.data
        } else {
            &self.inner
        }
    }

    pub fn mip_level_count(&self) -> u32 {
        if let Some(mips) = &self.mips {
            mips.levels
        } else {
            1
        }
    }
}

struct Mips {
    levels: u32,
    data: Vec<u8>,
}
