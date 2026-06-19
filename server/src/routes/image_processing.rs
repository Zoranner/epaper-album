use std::io::Cursor;

use image::{
    codecs::bmp::BmpEncoder, imageops::FilterType, DynamicImage, ExtendedColorType, GenericImage,
    GenericImageView, ImageEncoder, Rgba,
};

use crate::error::AppError;

pub(super) const DISPLAY_WIDTH: u32 = 800;
pub(super) const DISPLAY_HEIGHT: u32 = 480;

#[derive(Debug, Clone, Copy)]
pub(super) enum UploadedImageFormat {
    Bmp,
    Jpeg,
    Png,
}

impl UploadedImageFormat {
    pub(super) const fn extension(self) -> &'static str {
        match self {
            Self::Bmp => "bmp",
            Self::Jpeg => "jpg",
            Self::Png => "png",
        }
    }
}

pub(super) fn render_display_bmp(bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let image = image::load_from_memory(bytes)?;
    let fitted = fit_to_display(image);
    let paletted = quantize_six_color(fitted);
    encode_rgb_bmp(&paletted.to_rgb8())
}

pub(super) fn encode_rgb_bmp(image: &image::RgbImage) -> anyhow::Result<Vec<u8>> {
    let mut output = Cursor::new(Vec::new());
    let encoder = BmpEncoder::new(&mut output);
    encoder.write_image(
        image.as_raw(),
        image.width(),
        image.height(),
        ExtendedColorType::Rgb8,
    )?;
    Ok(output.into_inner())
}

pub(super) fn detect_uploaded_image_format(bytes: &[u8]) -> Result<UploadedImageFormat, AppError> {
    if bytes.starts_with(b"BM") {
        return Ok(UploadedImageFormat::Bmp);
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Ok(UploadedImageFormat::Jpeg);
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Ok(UploadedImageFormat::Png);
    }
    Err(AppError::BadRequest(
        "图片格式支持 JPG、PNG 和 BMP".to_string(),
    ))
}

fn fit_to_display(image: DynamicImage) -> DynamicImage {
    let (width, height) = image.dimensions();
    let scale = (DISPLAY_WIDTH as f32 / width as f32).max(DISPLAY_HEIGHT as f32 / height as f32);
    let resized_width = (width as f32 * scale).round() as u32;
    let resized_height = (height as f32 * scale).round() as u32;
    let resized = image.resize_exact(resized_width, resized_height, FilterType::Triangle);
    let left = (resized_width.saturating_sub(DISPLAY_WIDTH)) / 2;
    let top = (resized_height.saturating_sub(DISPLAY_HEIGHT)) / 2;
    resized.crop_imm(left, top, DISPLAY_WIDTH, DISPLAY_HEIGHT)
}

fn quantize_six_color(image: DynamicImage) -> DynamicImage {
    let palette = [
        Rgba([0, 0, 0, 255]),
        Rgba([255, 255, 255, 255]),
        Rgba([255, 0, 0, 255]),
        Rgba([255, 255, 0, 255]),
        Rgba([0, 0, 255, 255]),
        Rgba([0, 255, 0, 255]),
    ];
    let mut output = DynamicImage::new_rgba8(DISPLAY_WIDTH, DISPLAY_HEIGHT);
    let mut work = image.to_rgba8();

    for y in 0..DISPLAY_HEIGHT {
        for x in 0..DISPLAY_WIDTH {
            let pixel = *work.get_pixel(x, y);
            let nearest = palette
                .iter()
                .copied()
                .min_by_key(|candidate| color_distance(pixel, *candidate))
                .unwrap_or(palette[0]);
            output.put_pixel(x, y, nearest);
            diffuse_error(&mut work, x, y, pixel, nearest);
        }
    }

    output
}

fn diffuse_error(
    image: &mut image::RgbaImage,
    x: u32,
    y: u32,
    current: Rgba<u8>,
    quantized: Rgba<u8>,
) {
    let error = [
        current[0] as i16 - quantized[0] as i16,
        current[1] as i16 - quantized[1] as i16,
        current[2] as i16 - quantized[2] as i16,
    ];
    add_error(image, x as i32 + 1, y as i32, error, 7);
    add_error(image, x as i32 - 1, y as i32 + 1, error, 3);
    add_error(image, x as i32, y as i32 + 1, error, 5);
    add_error(image, x as i32 + 1, y as i32 + 1, error, 1);
}

fn add_error(image: &mut image::RgbaImage, x: i32, y: i32, error: [i16; 3], weight: i16) {
    if x < 0 || y < 0 || x >= DISPLAY_WIDTH as i32 || y >= DISPLAY_HEIGHT as i32 {
        return;
    }

    let pixel = image.get_pixel_mut(x as u32, y as u32);
    for channel in 0..3 {
        let value = pixel[channel] as i16 + error[channel] * weight / 16;
        pixel[channel] = value.clamp(0, 255) as u8;
    }
}

fn color_distance(left: Rgba<u8>, right: Rgba<u8>) -> u32 {
    left.0[..3]
        .iter()
        .zip(right.0[..3].iter())
        .map(|(left, right)| {
            let diff = *left as i32 - *right as i32;
            (diff * diff) as u32
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_display_bmp_outputs_device_compatible_bmp() {
        let mut input = image::RgbImage::new(32, 32);
        for (x, y, pixel) in input.enumerate_pixels_mut() {
            *pixel = image::Rgb([(x * 8) as u8, (y * 8) as u8, 128]);
        }

        let mut encoded_input = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(input)
            .write_to(&mut encoded_input, image::ImageFormat::Png)
            .expect("encode input png");

        let bmp = render_display_bmp(&encoded_input.into_inner()).expect("render display bmp");

        assert_eq!(&bmp[0..2], b"BM");
        assert_eq!(u32::from_le_bytes(bmp[14..18].try_into().unwrap()), 40);
        assert_eq!(i32::from_le_bytes(bmp[18..22].try_into().unwrap()), 800);
        assert_eq!(i32::from_le_bytes(bmp[22..26].try_into().unwrap()), 480);
        assert_eq!(u16::from_le_bytes(bmp[26..28].try_into().unwrap()), 1);
        assert_eq!(u16::from_le_bytes(bmp[28..30].try_into().unwrap()), 24);
        assert_eq!(u32::from_le_bytes(bmp[30..34].try_into().unwrap()), 0);
    }
}
