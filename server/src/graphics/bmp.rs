use std::io::Cursor;

use image::{codecs::bmp::BmpEncoder, ExtendedColorType, ImageEncoder};

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
