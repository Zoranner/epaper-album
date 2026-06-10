use crate::display::Color;
use crate::epd::{pack_epd_pixels, EPD_HEIGHT, EPD_ROW_BYTES, EPD_WIDTH};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BmpError {
    InvalidHeader,
    UnsupportedFormat,
    UnsupportedColor,
    UnexpectedEof,
}

impl fmt::Display for BmpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHeader => write!(f, "invalid-header"),
            Self::UnsupportedFormat => write!(f, "unsupported-format"),
            Self::UnsupportedColor => write!(f, "unsupported-color"),
            Self::UnexpectedEof => write!(f, "unexpected-eof"),
        }
    }
}

impl std::error::Error for BmpError {}

pub struct BmpImage<'a> {
    data: &'a [u8],
    pixel_offset: usize,
    width: usize,
    height: usize,
    row_stride: usize,
    top_down: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BmpHeader {
    pub pixel_offset: u64,
    pub width: usize,
    pub height: usize,
    pub row_stride: usize,
    pub top_down: bool,
}

impl<'a> BmpImage<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, BmpError> {
        let header = parse_bmp_header(data, data.len() as u64)?;
        let required_len = header.pixel_offset as usize + header.row_stride * header.height;
        if data.len() < required_len {
            return Err(BmpError::UnexpectedEof);
        }

        Ok(Self {
            data,
            pixel_offset: header.pixel_offset as usize,
            width: header.width,
            height: header.height,
            row_stride: header.row_stride,
            top_down: header.top_down,
        })
    }

    pub const fn width(&self) -> usize {
        self.width
    }

    pub const fn height(&self) -> usize {
        self.height
    }

    pub fn fill_epd_row(
        &self,
        source_y: usize,
        output: &mut [u8; EPD_ROW_BYTES],
    ) -> Result<(), BmpError> {
        if self.width != EPD_WIDTH || self.height != EPD_HEIGHT {
            return Err(BmpError::UnsupportedFormat);
        }

        for (x_pair, byte) in output.iter_mut().enumerate() {
            let left = self.color_at(x_pair * 2, source_y)?;
            let right = self.color_at(x_pair * 2 + 1, source_y)?;
            *byte = pack_epd_pixels(left, right);
        }

        Ok(())
    }

    pub fn color_at(&self, x: usize, y: usize) -> Result<Color, BmpError> {
        let (red, green, blue) = self.rgb_at(x, y)?;
        exact_panel_color(red, green, blue).ok_or(BmpError::UnsupportedColor)
    }

    pub fn rgb_at(&self, x: usize, y: usize) -> Result<(u8, u8, u8), BmpError> {
        if x >= self.width || y >= self.height {
            return Err(BmpError::UnexpectedEof);
        }

        let row = if self.top_down {
            y
        } else {
            self.height - 1 - y
        };
        let row_offset = self.pixel_offset + row * self.row_stride;
        let pixel_offset = row_offset + x * 3;
        let blue = self.data[pixel_offset];
        let green = self.data[pixel_offset + 1];
        let red = self.data[pixel_offset + 2];

        Ok((red, green, blue))
    }
}

pub fn parse_bmp_header(data: &[u8], total_len: u64) -> Result<BmpHeader, BmpError> {
    if data.len() < 54 || &data[0..2] != b"BM" {
        return Err(BmpError::InvalidHeader);
    }

    let pixel_offset = read_u32(data, 10)? as u64;
    let dib_size = read_u32(data, 14)?;
    let width = read_i32(data, 18)?;
    let height = read_i32(data, 22)?;
    let planes = read_u16(data, 26)?;
    let bit_count = read_u16(data, 28)?;
    let compression = read_u32(data, 30)?;

    if dib_size < 40
        || width <= 0
        || height == 0
        || planes != 1
        || bit_count != 24
        || compression != 0
    {
        return Err(BmpError::UnsupportedFormat);
    }

    let top_down = height < 0;
    let width = width as usize;
    let height = height.unsigned_abs() as usize;
    let row_stride = bmp_row_stride(width);
    let required_len = pixel_offset + (row_stride * height) as u64;
    if total_len < required_len {
        return Err(BmpError::UnexpectedEof);
    }

    Ok(BmpHeader {
        pixel_offset,
        width,
        height,
        row_stride,
        top_down,
    })
}

pub fn fill_epd_row_from_bgr(
    row_bgr: &[u8],
    output: &mut [u8; EPD_ROW_BYTES],
) -> Result<(), BmpError> {
    if row_bgr.len() < EPD_WIDTH * 3 {
        return Err(BmpError::UnexpectedEof);
    }

    for (x_pair, byte) in output.iter_mut().enumerate() {
        let left = bgr_pixel_color(row_bgr, x_pair * 2)?;
        let right = bgr_pixel_color(row_bgr, x_pair * 2 + 1)?;
        *byte = pack_epd_pixels(left, right);
    }

    Ok(())
}

pub fn fill_epd_row_from_bgr_mirrored(
    row_bgr: &[u8],
    output: &mut [u8; EPD_ROW_BYTES],
) -> Result<(), BmpError> {
    if row_bgr.len() < EPD_WIDTH * 3 {
        return Err(BmpError::UnexpectedEof);
    }

    for (x_pair, byte) in output.iter_mut().enumerate() {
        let left = bgr_pixel_color(row_bgr, EPD_WIDTH - 1 - x_pair * 2)?;
        let right = bgr_pixel_color(row_bgr, EPD_WIDTH - 2 - x_pair * 2)?;
        *byte = pack_epd_pixels(left, right);
    }

    Ok(())
}

pub const fn bmp_24bit_row_stride() -> usize {
    bmp_row_stride(EPD_WIDTH)
}

fn bgr_pixel_color(row_bgr: &[u8], x: usize) -> Result<Color, BmpError> {
    let pixel_offset = x * 3;
    let blue = row_bgr[pixel_offset];
    let green = row_bgr[pixel_offset + 1];
    let red = row_bgr[pixel_offset + 2];

    exact_panel_color(red, green, blue).ok_or(BmpError::UnsupportedColor)
}

pub const fn exact_panel_color(red: u8, green: u8, blue: u8) -> Option<Color> {
    match (red, green, blue) {
        (255, 255, 255) => Some(Color::White),
        (0, 0, 0) => Some(Color::Black),
        (255, 255, 0) => Some(Color::Yellow),
        (255, 0, 0) => Some(Color::Red),
        (0, 0, 255) => Some(Color::Blue),
        (0, 255, 0) => Some(Color::Green),
        _ => None,
    }
}

pub const fn bmp_row_stride(width: usize) -> usize {
    (width * 3).div_ceil(4) * 4
}

fn read_u16(data: &[u8], offset: usize) -> Result<u16, BmpError> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or(BmpError::UnexpectedEof)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32, BmpError> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or(BmpError::UnexpectedEof)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_i32(data: &[u8], offset: usize) -> Result<i32, BmpError> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or(BmpError::UnexpectedEof)?;
    Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_exact_panel_colors() {
        assert_eq!(exact_panel_color(255, 255, 255), Some(Color::White));
        assert_eq!(exact_panel_color(0, 0, 0), Some(Color::Black));
        assert_eq!(exact_panel_color(255, 255, 0), Some(Color::Yellow));
        assert_eq!(exact_panel_color(255, 0, 0), Some(Color::Red));
        assert_eq!(exact_panel_color(0, 0, 255), Some(Color::Blue));
        assert_eq!(exact_panel_color(0, 255, 0), Some(Color::Green));
        assert_eq!(exact_panel_color(128, 128, 128), None);
    }

    #[test]
    fn parses_800_by_480_24bit_bmp_header() {
        let bmp = sample_bmp(false);
        let image = BmpImage::parse(&bmp).unwrap();

        let mut row = [0u8; EPD_ROW_BYTES];
        image.fill_epd_row(0, &mut row).unwrap();

        assert_eq!(row[0], pack_epd_pixels(Color::Red, Color::Green));
    }

    #[test]
    fn parses_streaming_bmp_header() {
        let bmp = sample_bmp(false);
        let header = parse_bmp_header(&bmp[..54], bmp.len() as u64).unwrap();

        assert_eq!(header.pixel_offset, 54);
        assert_eq!(header.width, EPD_WIDTH);
        assert_eq!(header.height, EPD_HEIGHT);
        assert_eq!(header.row_stride, bmp_24bit_row_stride());
        assert!(!header.top_down);
    }

    #[test]
    fn parses_small_sprite_bmp() {
        let bmp = sample_sized_bmp(3, 2, true);
        let image = BmpImage::parse(&bmp).unwrap();

        assert_eq!(image.width(), 3);
        assert_eq!(image.height(), 2);
        assert_eq!(image.color_at(0, 0), Ok(Color::Red));
        assert_eq!(image.color_at(1, 0), Ok(Color::Green));
        assert_eq!(
            image.fill_epd_row(0, &mut [0u8; EPD_ROW_BYTES]),
            Err(BmpError::UnsupportedFormat)
        );
    }

    #[test]
    fn fills_epd_row_from_bgr_palette_pixels() {
        let mut row_bgr = vec![0u8; bmp_24bit_row_stride()];
        row_bgr[0..3].copy_from_slice(&[0, 0, 255]);
        row_bgr[3..6].copy_from_slice(&[0, 255, 0]);

        let mut row = [0u8; EPD_ROW_BYTES];
        fill_epd_row_from_bgr(&row_bgr, &mut row).unwrap();

        assert_eq!(row[0], pack_epd_pixels(Color::Red, Color::Green));
    }

    #[test]
    fn fills_epd_row_from_bgr_mirrored_palette_pixels() {
        let mut row_bgr = vec![0u8; bmp_24bit_row_stride()];
        let last = (EPD_WIDTH - 1) * 3;
        let before_last = (EPD_WIDTH - 2) * 3;
        row_bgr[last..last + 3].copy_from_slice(&[0, 0, 255]);
        row_bgr[before_last..before_last + 3].copy_from_slice(&[0, 255, 0]);

        let mut row = [0u8; EPD_ROW_BYTES];
        fill_epd_row_from_bgr_mirrored(&row_bgr, &mut row).unwrap();

        assert_eq!(row[0], pack_epd_pixels(Color::Red, Color::Green));
    }

    #[test]
    fn supports_top_down_bmp_header() {
        let bmp = sample_bmp(true);
        let image = BmpImage::parse(&bmp).unwrap();

        let mut row = [0u8; EPD_ROW_BYTES];
        image.fill_epd_row(0, &mut row).unwrap();

        assert_eq!(row[0], pack_epd_pixels(Color::Red, Color::Green));
    }

    #[test]
    fn rejects_non_palette_pixels() {
        let mut bmp = sample_bmp(false);
        let row_offset = 54 + (EPD_HEIGHT - 1) * bmp_row_stride(EPD_WIDTH);
        bmp[row_offset..row_offset + 3].copy_from_slice(&[128, 128, 128]);
        let image = BmpImage::parse(&bmp).unwrap();

        let mut row = [0u8; EPD_ROW_BYTES];
        let result = image.fill_epd_row(0, &mut row);

        assert_eq!(result, Err(BmpError::UnsupportedColor));
    }

    fn sample_bmp(top_down: bool) -> Vec<u8> {
        sample_sized_bmp(EPD_WIDTH, EPD_HEIGHT, top_down)
    }

    fn sample_sized_bmp(width: usize, height: usize, top_down: bool) -> Vec<u8> {
        let pixel_offset = 54usize;
        let row_stride = bmp_row_stride(width);
        let file_size = pixel_offset + row_stride * height;
        let mut bmp = vec![0u8; file_size];
        bmp[0..2].copy_from_slice(b"BM");
        bmp[2..6].copy_from_slice(&(file_size as u32).to_le_bytes());
        bmp[10..14].copy_from_slice(&(pixel_offset as u32).to_le_bytes());
        bmp[14..18].copy_from_slice(&40u32.to_le_bytes());
        bmp[18..22].copy_from_slice(&(width as i32).to_le_bytes());
        let signed_height = if top_down {
            -(height as i32)
        } else {
            height as i32
        };
        bmp[22..26].copy_from_slice(&signed_height.to_le_bytes());
        bmp[26..28].copy_from_slice(&1u16.to_le_bytes());
        bmp[28..30].copy_from_slice(&24u16.to_le_bytes());

        let file_row = if top_down { 0 } else { height - 1 };
        let row_offset = pixel_offset + file_row * row_stride;
        bmp[row_offset..row_offset + 3].copy_from_slice(&[0, 0, 255]);
        bmp[row_offset + 3..row_offset + 6].copy_from_slice(&[0, 255, 0]);
        bmp
    }
}
