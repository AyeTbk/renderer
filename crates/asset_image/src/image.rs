use std::path::Path;

pub struct Image {
    inner: image::RgbaImage,
    mips: Option<Mips>,
}

impl Image {
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, String> {
        let dyn_image = image::open(path).map_err(|e| format!("{:?}", e))?;
        Ok(Self::from_dynamic_image(dyn_image))
    }

    pub fn load_from_memory(data: &[u8]) -> Result<Self, String> {
        let dyn_image = image::load_from_memory(data).map_err(|e| format!("{:?}", e))?;
        Ok(Self::from_dynamic_image(dyn_image))
    }

    pub fn new_dummy() -> Self {
        let inner = image::RgbaImage::from_pixel(1, 1, image::Rgba([128, 128, 128, 255]));
        Self {
            inner: inner.into(),
            mips: None,
        }
    }

    fn from_dynamic_image(dyn_image: image::DynamicImage) -> Self {
        Self {
            inner: dyn_image.into_rgba8(),
            mips: None,
        }
    }

    pub fn make_mips(&mut self) -> Result<(), String> {
        if !self.width().is_power_of_two() || !(self.height() == self.width()) {
            return Err(format!("can't generate mipmaps on images that aren't square and that have non power of two dimensions: dimensions {}x{}", self.width(), self.height()));
        }

        fn make_mips<const N: usize>(src_pixel_width: usize, src_data: &[u8]) -> Mips {
            let max_level: Option<usize> = None;

            fn mip_size_from_level(level0_size: usize, level: usize) -> usize {
                level0_size >> level
            }
            fn data_size_from_mip_size(size: usize, pixel_byte_size: usize) -> usize {
                size * size * pixel_byte_size
            }

            let level0_size = src_pixel_width;
            let level_count =
                (level0_size.ilog2() as usize + 1).min(max_level.unwrap_or(usize::MAX));

            let mut data_size: usize = 0;
            for level in 0..level_count {
                let level_size = mip_size_from_level(level0_size, level);
                data_size += data_size_from_mip_size(level_size, N);
            }

            let mut data: Vec<u8> = Vec::new();
            data.resize(data_size, 128);

            // level 0:
            let level0_buffer_len = src_data.len();
            data[..level0_buffer_len].copy_from_slice(src_data);

            // level 1..n:
            let (mut prev_mip_buffer, mut data_view) = data.split_at_mut(level0_buffer_len);
            let mut prev_level_size = level0_size;
            for level in 1..level_count {
                let level_size = mip_size_from_level(level0_size, level);
                let mip_buffer_len = data_size_from_mip_size(level_size, N);
                let (mip_buffer, after) = data_view.split_at_mut(mip_buffer_len);
                data_view = after;

                downsample_bilinear::<N>(prev_level_size, prev_mip_buffer, mip_buffer);

                prev_mip_buffer = mip_buffer;
                prev_level_size = level_size;
            }

            Mips {
                level_count: level_count as u32,
                data,
            }
        }

        self.mips = Some(make_mips::<4>(self.inner.width() as usize, self.data()));
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
            mips.level_count
        } else {
            1
        }
    }
}

struct Mips {
    level_count: u32,
    data: Vec<u8>,
}

fn downsample_bilinear<const N: usize>(src_size: usize, src: &[u8], dst: &mut [u8]) {
    let expected_src_len = src_size * src_size * N;
    let dst_size = src_size / 2;
    let expected_dst_len = dst_size * dst_size * N;
    assert_eq!(src.len(), expected_src_len);
    assert_eq!(dst.len(), expected_dst_len);

    fn get_pixel(buf: &[u8], x: usize, y: usize, size: usize) -> [u8; 4] {
        let row_byte_count = size * 4;
        let idx = y * row_byte_count + x * 4;
        let mut result = [0u8; 4];
        for i in 0..4 {
            result[i] = buf[idx + i];
        }
        result
    }
    fn set_pixel(buf: &mut [u8], x: usize, y: usize, size: usize, pixel: [u8; 4]) {
        let row_byte_count = size * 4;
        let idx = y * row_byte_count + x * 4;
        for i in 0..4 {
            buf[idx + i] = pixel[i];
        }
    }

    fn srgb_to_rgb(color: [u8; 4]) -> [f32; 4] {
        [
            (color[0] as f32 / 255.0).powf(2.2),
            (color[1] as f32 / 255.0).powf(2.2),
            (color[2] as f32 / 255.0).powf(2.2),
            color[3] as f32 / 255.0,
        ]
    }
    fn rgb_to_srgb(color: [f32; 4]) -> [u8; 4] {
        [
            (color[0].powf(1.0 / 2.2) * 255.0) as u8,
            (color[1].powf(1.0 / 2.2) * 255.0) as u8,
            (color[2].powf(1.0 / 2.2) * 255.0) as u8,
            (color[3] * 255.0) as u8,
        ]
    }

    for y in 0..dst_size {
        for x in 0..dst_size {
            let src_x1 = x * 2;
            let src_y1 = y * 2;

            let src_x2 = src_x1 + 1;
            let src_y2 = src_y1;

            let src_x3 = src_x1;
            let src_y3 = src_y1 + 1;

            let src_x4 = src_x1 + 1;
            let src_y4 = src_y1 + 1;

            let src1 = srgb_to_rgb(get_pixel(src, src_x1, src_y1, src_size));
            let src2 = srgb_to_rgb(get_pixel(src, src_x2, src_y2, src_size));
            let src3 = srgb_to_rgb(get_pixel(src, src_x3, src_y3, src_size));
            let src4 = srgb_to_rgb(get_pixel(src, src_x4, src_y4, src_size));

            let average = average([src1, src2, src3, src4]);

            set_pixel(dst, x, y, dst_size, rgb_to_srgb(average));
        }
    }
}

fn average<const N: usize, const M: usize>(list: [[f32; N]; M]) -> [f32; N] {
    let mut sum = [0f32; N];
    for arr in list {
        for i in 0..N {
            sum[i] += arr[i];
        }
    }
    let mut result = [0f32; N];
    for i in 0..N {
        result[i] = sum[i] / M as f32;
    }
    result
}
