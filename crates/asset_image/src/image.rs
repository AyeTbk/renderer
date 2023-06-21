use std::path::Path;

const PIXEL_BYTE_SIZE: usize = 4;

pub struct Image {
    inner: image::RgbaImage,
    mips: Option<Mips>,
}

impl Image {
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, String> {
        let dyn_image = image::open(path).map_err(|e| format!("{:?}", e))?;
        Ok(Self {
            inner: dyn_image.into_rgba8(),
            mips: None,
        })
    }

    pub fn load_from_memory(data: &[u8]) -> Result<Self, String> {
        let dyn_image = image::load_from_memory(data).map_err(|e| format!("{:?}", e))?;
        Ok(Self {
            inner: dyn_image.into_rgba8(),
            mips: None,
        })
    }

    pub fn make_mips(&mut self) -> Result<(), String> {
        if !self.width().is_power_of_two() || !(self.height() == self.width()) {
            return Err(format!("can't generate mipmaps on images that aren't square and that have non power of two dimensions: dimensions {}x{}", self.width(), self.height()));
        }

        let max_level: Option<usize> = None;

        fn mip_size_from_level(level0_size: usize, level: usize) -> usize {
            level0_size >> level
        }
        fn data_size_from_mip_size(size: usize) -> usize {
            size * size * PIXEL_BYTE_SIZE
        }

        let level0_size = self.width() as usize;
        let level_count = (level0_size.ilog2() as usize + 1).min(max_level.unwrap_or(usize::MAX));

        let mut data_size: usize = 0;
        for level in 0..level_count {
            let level_size = mip_size_from_level(level0_size, level);
            data_size += data_size_from_mip_size(level_size);
        }

        let mut data: Vec<u8> = Vec::new();
        data.resize(data_size, 128);

        // level 0:
        let level0_buffer_len = self.inner.len();
        data[..level0_buffer_len].copy_from_slice(&self.inner);

        // level 1..n:
        let (mut prev_mip_buffer, mut data_view) = data.split_at_mut(level0_buffer_len);
        let mut prev_level_size = level0_size;
        for level in 1..level_count {
            let level_size = mip_size_from_level(level0_size, level);
            let mip_buffer_len = data_size_from_mip_size(level_size);
            let (mip_buffer, after) = data_view.split_at_mut(mip_buffer_len);
            data_view = after;

            downsample(prev_level_size, prev_mip_buffer, mip_buffer);

            prev_mip_buffer = mip_buffer;
            prev_level_size = level_size;
        }

        self.mips = Some(Mips {
            level_count: level_count as u32,
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

fn downsample(src_size: usize, src: &[u8], dst: &mut [u8]) {
    #[allow(unused)]
    #[derive(PartialEq)]
    enum Sampling {
        Correct,
        Fast,
    }
    let sampling = Sampling::Fast;

    if sampling == Sampling::Correct {
        downsample_srgb(src_size, src, dst);
    } else {
        downsample_linear(src_size, src, dst);
    }
}

fn downsample_srgb(src_size: usize, src: &[u8], dst: &mut [u8]) {
    let expected_src_len = src_size * src_size * PIXEL_BYTE_SIZE;
    let dst_size = src_size / 2;
    let expected_dst_len = dst_size * dst_size * PIXEL_BYTE_SIZE;
    assert_eq!(src.len(), expected_src_len);
    assert_eq!(dst.len(), expected_dst_len);

    fn get_pixel(buf: &[u8], x: usize, y: usize, size: usize) -> [u8; PIXEL_BYTE_SIZE] {
        let row_byte_count = size * PIXEL_BYTE_SIZE;
        let idx = y * row_byte_count + x * PIXEL_BYTE_SIZE;
        [buf[idx], buf[idx + 1], buf[idx + 2], buf[idx + 3]]
    }
    fn set_pixel(buf: &mut [u8], x: usize, y: usize, size: usize, pixel: [u8; PIXEL_BYTE_SIZE]) {
        let row_byte_count = size * PIXEL_BYTE_SIZE;
        let idx = y * row_byte_count + x * PIXEL_BYTE_SIZE;
        buf[idx] = pixel[0];
        buf[idx + 1] = pixel[1];
        buf[idx + 2] = pixel[2];
        buf[idx + 3] = pixel[3];
    }

    const GAMMA: f32 = 2.2;
    fn srgb_to_rgb(color: [u8; 4]) -> [f32; 4] {
        [
            ((color[0] as f32) / std::u8::MAX as f32).powf(GAMMA),
            ((color[1] as f32) / std::u8::MAX as f32).powf(GAMMA),
            ((color[2] as f32) / std::u8::MAX as f32).powf(GAMMA),
            (color[3] as f32) / std::u8::MAX as f32,
        ]
    }
    fn rgb_to_srgb(color: [f32; 4]) -> [u8; 4] {
        [
            (color[0].powf(1.0 / GAMMA) * std::u8::MAX as f32) as u8,
            (color[1].powf(1.0 / GAMMA) * std::u8::MAX as f32) as u8,
            (color[2].powf(1.0 / GAMMA) * std::u8::MAX as f32) as u8,
            (color[3] * std::u8::MAX as f32) as u8,
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

            let average = [
                (src1[0] + src2[0] + src3[0] + src4[0]) / 4.0, //
                (src1[1] + src2[1] + src3[1] + src4[1]) / 4.0, //
                (src1[2] + src2[2] + src3[2] + src4[2]) / 4.0, //
                (src1[3] + src2[3] + src3[3] + src4[3]) / 4.0, //
            ];

            set_pixel(dst, x, y, dst_size, rgb_to_srgb(average));
        }
    }
}

fn downsample_linear(src_size: usize, src: &[u8], dst: &mut [u8]) {
    let expected_src_len = src_size * src_size * PIXEL_BYTE_SIZE;
    let dst_size = src_size / 2;
    let expected_dst_len = dst_size * dst_size * PIXEL_BYTE_SIZE;
    assert_eq!(src.len(), expected_src_len);
    assert_eq!(dst.len(), expected_dst_len);

    fn get_pixel(buf: &[u8], x: usize, y: usize, size: usize) -> [u8; PIXEL_BYTE_SIZE] {
        let row_byte_count = size * PIXEL_BYTE_SIZE;
        let idx = y * row_byte_count + x * PIXEL_BYTE_SIZE;
        [buf[idx], buf[idx + 1], buf[idx + 2], buf[idx + 3]]
    }
    fn set_pixel(buf: &mut [u8], x: usize, y: usize, size: usize, pixel: [u8; PIXEL_BYTE_SIZE]) {
        let row_byte_count = size * PIXEL_BYTE_SIZE;
        let idx = y * row_byte_count + x * PIXEL_BYTE_SIZE;
        buf[idx] = pixel[0];
        buf[idx + 1] = pixel[1];
        buf[idx + 2] = pixel[2];
        buf[idx + 3] = pixel[3];
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

            let src1 = get_pixel(src, src_x1, src_y1, src_size);
            let src2 = get_pixel(src, src_x2, src_y2, src_size);
            let src3 = get_pixel(src, src_x3, src_y3, src_size);
            let src4 = get_pixel(src, src_x4, src_y4, src_size);

            let average = [
                ((src1[0] as u16 + src2[0] as u16 + src3[0] as u16 + src4[0] as u16) / 4) as u8, //
                ((src1[1] as u16 + src2[1] as u16 + src3[1] as u16 + src4[1] as u16) / 4) as u8, //
                ((src1[2] as u16 + src2[2] as u16 + src3[2] as u16 + src4[2] as u16) / 4) as u8, //
                ((src1[3] as u16 + src2[3] as u16 + src3[3] as u16 + src4[3] as u16) / 4) as u8, //
            ];

            set_pixel(dst, x, y, dst_size, average);
        }
    }
}
