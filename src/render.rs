use crate::bmp::{BmpError, BmpImage};
use crate::display::{Color, ScreenBuffer, SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::epd::{pack_epd_pixels, set_logical_packed_frame_pixel, EPD_FRAME_BYTES};
use std::fmt;

const PLACEHOLDER_GLYPH_WIDTH: usize = 5;
const PLACEHOLDER_GLYPH_HEIGHT: usize = 7;
const GLYPH_GAP: usize = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextStyle {
    pub foreground: Color,
    pub background: Color,
    pub padding_x: usize,
    pub padding_y: usize,
    pub margin_x: usize,
    pub margin_y: usize,
    pub glyph_width: usize,
    pub glyph_height: usize,
    pub glyph_gap: usize,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            foreground: Color::Black,
            background: Color::White,
            padding_x: 8,
            padding_y: 6,
            margin_x: 12,
            margin_y: 12,
            glyph_width: PLACEHOLDER_GLYPH_WIDTH,
            glyph_height: PLACEHOLDER_GLYPH_HEIGHT,
            glyph_gap: GLYPH_GAP,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OverlaySlot {
    BottomLeft,
    BottomRight,
    TopLeft,
    TopRight,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderError {
    InvalidPhotoSize,
    InvalidFrameLength,
    Bmp(BmpError),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPhotoSize => write!(f, "invalid-photo-size"),
            Self::InvalidFrameLength => write!(f, "invalid-frame-length"),
            Self::Bmp(error) => write!(f, "bmp-{error}"),
        }
    }
}

impl std::error::Error for RenderError {}

impl From<BmpError> for RenderError {
    fn from(error: BmpError) -> Self {
        Self::Bmp(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderNotice {
    LowBattery,
    Offline,
    SyncFailed,
    PlanExpired,
    StorageLow,
}

impl RenderNotice {
    pub const fn text(self) -> &'static str {
        match self {
            Self::LowBattery => "LOW BAT",
            Self::Offline => "OFFLINE",
            Self::SyncFailed => "SYNC FAIL",
            Self::PlanExpired => "EXPIRED",
            Self::StorageLow => "STORAGE LOW",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SpriteBmps<'a> {
    pub caption: Option<&'a [u8]>,
    pub date: Option<&'a [u8]>,
    pub notice: Option<&'a [u8]>,
    pub status: Option<&'a [u8]>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SpritePlacement {
    pub margin_x: usize,
    pub margin_y: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackedFrameRenderInput<'a> {
    pub photo_bmp: &'a [u8],
    pub sprites: SpriteBmps<'a>,
    pub placement: SpritePlacement,
}

impl<'a> PackedFrameRenderInput<'a> {
    pub const fn new(photo_bmp: &'a [u8]) -> Self {
        Self {
            photo_bmp,
            sprites: SpriteBmps {
                caption: None,
                date: None,
                notice: None,
                status: None,
            },
            placement: SpritePlacement {
                margin_x: 0,
                margin_y: 0,
            },
        }
    }

    pub const fn with_sprites(mut self, sprites: SpriteBmps<'a>) -> Self {
        self.sprites = sprites;
        self
    }

    pub const fn with_placement(mut self, placement: SpritePlacement) -> Self {
        self.placement = placement;
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RenderImage<'a> {
    pub pixels: &'a [Color],
    pub width: usize,
    pub height: usize,
}

impl<'a> RenderImage<'a> {
    pub const fn new(pixels: &'a [Color], width: usize, height: usize) -> Self {
        Self {
            pixels,
            width,
            height,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RenderInput<'a> {
    pub image: Option<RenderImage<'a>>,
    pub caption: &'a str,
    pub date: &'a str,
    pub notice: Option<RenderNotice>,
    pub style: TextStyle,
}

impl<'a> RenderInput<'a> {
    pub const fn new(caption: &'a str, date: &'a str) -> Self {
        Self {
            image: None,
            caption,
            date,
            notice: None,
            style: TextStyle {
                foreground: Color::Black,
                background: Color::White,
                padding_x: 8,
                padding_y: 6,
                margin_x: 12,
                margin_y: 12,
                glyph_width: PLACEHOLDER_GLYPH_WIDTH,
                glyph_height: PLACEHOLDER_GLYPH_HEIGHT,
                glyph_gap: GLYPH_GAP,
            },
        }
    }

    pub const fn with_image(mut self, image: RenderImage<'a>) -> Self {
        self.image = Some(image);
        self
    }

    pub const fn with_notice(mut self, notice: RenderNotice) -> Self {
        self.notice = Some(notice);
        self
    }

    pub const fn with_style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }
}

pub fn render_photo_page(input: &RenderInput<'_>) -> ScreenBuffer {
    let mut buffer = ScreenBuffer::default();
    render_into(&mut buffer, input);
    buffer
}

pub fn render_epd_packed_frame_from_bmps(
    input: &PackedFrameRenderInput<'_>,
) -> Result<Vec<u8>, RenderError> {
    let mut frame = vec![pack_epd_pixels(Color::White, Color::White); EPD_FRAME_BYTES];
    render_epd_packed_frame_from_bmps_into(&mut frame, input)?;
    Ok(frame)
}

pub fn render_epd_packed_frame_from_bmps_into(
    frame: &mut [u8],
    input: &PackedFrameRenderInput<'_>,
) -> Result<(), RenderError> {
    if frame.len() != EPD_FRAME_BYTES {
        return Err(RenderError::InvalidFrameLength);
    }

    let photo = BmpImage::parse(input.photo_bmp)?;
    if photo.width() != SCREEN_WIDTH || photo.height() != SCREEN_HEIGHT {
        return Err(RenderError::InvalidPhotoSize);
    }

    draw_bmp_at(frame, &photo, 0, 0)?;

    if let Some(caption) = input.sprites.caption {
        draw_sprite_bmp(frame, caption, OverlaySlot::BottomLeft, input.placement)?;
    }

    if let Some(date) = input.sprites.date {
        draw_sprite_bmp(frame, date, OverlaySlot::BottomRight, input.placement)?;
    }

    if let Some(notice) = input.sprites.notice {
        draw_sprite_bmp(frame, notice, OverlaySlot::TopLeft, input.placement)?;
    }

    Ok(())
}

pub fn render_into(buffer: &mut ScreenBuffer, input: &RenderInput<'_>) {
    buffer.clear(Color::White);

    if let Some(image) = input.image {
        draw_centered_image(buffer, image, Color::White);
    }

    if !input.caption.is_empty() {
        draw_text(buffer, OverlaySlot::BottomLeft, input.caption, &input.style);
    }

    if !input.date.is_empty() {
        draw_text(buffer, OverlaySlot::BottomRight, input.date, &input.style);
    }

    if let Some(notice) = input.notice {
        draw_text(buffer, OverlaySlot::TopLeft, notice.text(), &input.style);
    }
}

pub fn draw_solid_block(
    buffer: &mut ScreenBuffer,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    color: Color,
) {
    buffer.fill_rect(x, y, width, height, color);
}

pub fn draw_text(buffer: &mut ScreenBuffer, slot: OverlaySlot, text: &str, style: &TextStyle) {
    let Some((block_width, block_height)) = text_size(text, style) else {
        return;
    };

    let (x, y) = overlay_origin(slot, block_width, block_height, style);
    draw_text_at(buffer, x, y, text, style);
}

pub fn draw_text_at(buffer: &mut ScreenBuffer, x: usize, y: usize, text: &str, style: &TextStyle) {
    let Some((block_width, block_height)) = text_size(text, style) else {
        return;
    };

    buffer.fill_rect(x, y, block_width, block_height, style.background);

    let mut cursor_x = x.saturating_add(style.padding_x);
    let glyph_y = y.saturating_add(style.padding_y);
    for character in text.chars() {
        if character.is_whitespace() {
            cursor_x = cursor_x
                .saturating_add(style.glyph_width)
                .saturating_add(style.glyph_gap);
            continue;
        }

        draw_glyph(
            buffer,
            cursor_x,
            glyph_y,
            character,
            style.glyph_width,
            style.glyph_height,
            style.foreground,
        );
        cursor_x = cursor_x
            .saturating_add(style.glyph_width)
            .saturating_add(style.glyph_gap);
    }
}

pub fn draw_placeholder_text(
    buffer: &mut ScreenBuffer,
    slot: OverlaySlot,
    text: &str,
    style: &TextStyle,
) {
    draw_text(buffer, slot, text, style);
}

fn draw_glyph(
    buffer: &mut ScreenBuffer,
    x: usize,
    y: usize,
    character: char,
    width: usize,
    height: usize,
    color: Color,
) {
    if width == 0 || height == 0 {
        return;
    }

    let pattern = glyph_pattern(character);
    for glyph_y in 0..height {
        let source_y = glyph_y * PLACEHOLDER_GLYPH_HEIGHT / height;
        let row = pattern[source_y];
        for glyph_x in 0..width {
            let source_x = glyph_x * PLACEHOLDER_GLYPH_WIDTH / width;
            let bit = 1 << (PLACEHOLDER_GLYPH_WIDTH - 1 - source_x);
            if row & bit != 0 {
                buffer.set_pixel(x + glyph_x, y + glyph_y, color);
            }
        }
    }
}

pub fn glyph_pattern(character: char) -> [u8; PLACEHOLDER_GLYPH_HEIGHT] {
    match character.to_ascii_uppercase() {
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        '3' => [0x1E, 0x01, 0x01, 0x0E, 0x01, 0x01, 0x1E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x10, 0x1E, 0x01, 0x01, 0x1E],
        '6' => [0x0E, 0x10, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x01, 0x0E],
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x12, 0x12, 0x0C],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        '-' => [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00],
        ':' => [0x00, 0x04, 0x04, 0x00, 0x04, 0x04, 0x00],
        '/' => [0x01, 0x01, 0x02, 0x04, 0x08, 0x10, 0x10],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
        _ => [0x1F, 0x11, 0x15, 0x15, 0x15, 0x11, 0x1F],
    }
}

fn draw_centered_image(buffer: &mut ScreenBuffer, image: RenderImage<'_>, background: Color) {
    if image.width == 0 || image.height == 0 {
        return;
    }

    let copy_width = image.width.min(SCREEN_WIDTH);
    let copy_height = image.height.min(SCREEN_HEIGHT);
    let source_x = image.width.saturating_sub(copy_width) / 2;
    let source_y = image.height.saturating_sub(copy_height) / 2;
    let target_x = SCREEN_WIDTH.saturating_sub(copy_width) / 2;
    let target_y = SCREEN_HEIGHT.saturating_sub(copy_height) / 2;

    buffer.clear(background);

    for row in 0..copy_height {
        let source_row = source_y + row;
        let target_row = target_y + row;
        for column in 0..copy_width {
            let source_index = source_row
                .saturating_mul(image.width)
                .saturating_add(source_x)
                .saturating_add(column);

            if let Some(color) = image.pixels.get(source_index).copied() {
                buffer.set_pixel(target_x + column, target_row, color);
            }
        }
    }
}

fn draw_sprite_bmp(
    frame: &mut [u8],
    sprite_bmp: &[u8],
    slot: OverlaySlot,
    placement: SpritePlacement,
) -> Result<(), RenderError> {
    let sprite = BmpImage::parse(sprite_bmp)?;
    let (x, y) = packed_overlay_origin(slot, sprite.width(), sprite.height(), placement);
    draw_bmp_at(frame, &sprite, x, y)
}

fn draw_bmp_at(
    frame: &mut [u8],
    image: &BmpImage<'_>,
    x: usize,
    y: usize,
) -> Result<(), RenderError> {
    for source_y in 0..image.height() {
        let target_y = y.saturating_add(source_y);
        if target_y >= SCREEN_HEIGHT {
            break;
        }

        for source_x in 0..image.width() {
            let target_x = x.saturating_add(source_x);
            if target_x >= SCREEN_WIDTH {
                break;
            }

            let color = image.color_at(source_x, source_y)?;
            if !set_logical_packed_frame_pixel(frame, target_x, target_y, color) {
                return Err(RenderError::InvalidFrameLength);
            }
        }
    }

    Ok(())
}

pub fn text_size(text: &str, style: &TextStyle) -> Option<(usize, usize)> {
    let glyph_count = text.chars().count();
    if glyph_count == 0 || style.glyph_width == 0 || style.glyph_height == 0 {
        return None;
    }

    let glyphs_width = glyph_count
        .saturating_mul(style.glyph_width)
        .saturating_add(
            glyph_count
                .saturating_sub(1)
                .saturating_mul(style.glyph_gap),
        );
    let width = glyphs_width.saturating_add(style.padding_x.saturating_mul(2));
    let height = style
        .glyph_height
        .saturating_add(style.padding_y.saturating_mul(2));

    Some((width.min(SCREEN_WIDTH), height.min(SCREEN_HEIGHT)))
}

fn overlay_origin(
    slot: OverlaySlot,
    block_width: usize,
    block_height: usize,
    style: &TextStyle,
) -> (usize, usize) {
    let left = style.margin_x.min(SCREEN_WIDTH.saturating_sub(block_width));
    let right = SCREEN_WIDTH
        .saturating_sub(style.margin_x)
        .saturating_sub(block_width);
    let top = style
        .margin_y
        .min(SCREEN_HEIGHT.saturating_sub(block_height));
    let bottom = SCREEN_HEIGHT
        .saturating_sub(style.margin_y)
        .saturating_sub(block_height);

    match slot {
        OverlaySlot::BottomLeft => (left, bottom),
        OverlaySlot::BottomRight => (right, bottom),
        OverlaySlot::TopLeft => (left, top),
        OverlaySlot::TopRight => (right, top),
    }
}

fn packed_overlay_origin(
    slot: OverlaySlot,
    block_width: usize,
    block_height: usize,
    placement: SpritePlacement,
) -> (usize, usize) {
    let left = placement
        .margin_x
        .min(SCREEN_WIDTH.saturating_sub(block_width));
    let right = SCREEN_WIDTH
        .saturating_sub(placement.margin_x)
        .saturating_sub(block_width);
    let top = placement
        .margin_y
        .min(SCREEN_HEIGHT.saturating_sub(block_height));
    let bottom = SCREEN_HEIGHT
        .saturating_sub(placement.margin_y)
        .saturating_sub(block_height);

    match slot {
        OverlaySlot::BottomLeft => (left, bottom),
        OverlaySlot::BottomRight => (right, bottom),
        OverlaySlot::TopLeft => (left, top),
        OverlaySlot::TopRight => (right, top),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bmp::bmp_row_stride;
    use crate::epd::{epd_color_code, EPD_HEIGHT, EPD_ROW_BYTES, EPD_WIDTH};

    #[test]
    fn overlays_sprites_at_formal_slots() {
        let photo = solid_bmp(EPD_WIDTH, EPD_HEIGHT, Color::White);
        let caption = solid_bmp(3, 2, Color::Black);
        let date = solid_bmp(4, 2, Color::Red);
        let notice = solid_bmp(2, 3, Color::Blue);

        let frame = render_epd_packed_frame_from_bmps(
            &PackedFrameRenderInput::new(&photo).with_sprites(SpriteBmps {
                caption: Some(&caption),
                date: Some(&date),
                notice: Some(&notice),
                status: None,
            }),
        )
        .unwrap();

        assert_eq!(logical_frame_color(&frame, 0, EPD_HEIGHT - 2), Color::Black);
        assert_eq!(logical_frame_color(&frame, 2, EPD_HEIGHT - 1), Color::Black);
        assert_eq!(
            logical_frame_color(&frame, EPD_WIDTH - 4, EPD_HEIGHT - 2),
            Color::Red
        );
        assert_eq!(
            logical_frame_color(&frame, EPD_WIDTH - 1, EPD_HEIGHT - 1),
            Color::Red
        );
        assert_eq!(logical_frame_color(&frame, 0, 0), Color::Blue);
        assert_eq!(logical_frame_color(&frame, 1, 2), Color::Blue);
    }

    #[test]
    fn white_sprite_background_overlays_photo_before_black_text() {
        let photo = solid_bmp(EPD_WIDTH, EPD_HEIGHT, Color::Green);
        let caption = bmp_from_pixels(
            2,
            2,
            &[Color::White, Color::Black, Color::White, Color::White],
        );

        let frame = render_epd_packed_frame_from_bmps(
            &PackedFrameRenderInput::new(&photo).with_sprites(SpriteBmps {
                caption: Some(&caption),
                date: None,
                notice: None,
                status: None,
            }),
        )
        .unwrap();

        assert_eq!(logical_frame_color(&frame, 0, EPD_HEIGHT - 2), Color::White);
        assert_eq!(logical_frame_color(&frame, 1, EPD_HEIGHT - 2), Color::Black);
        assert_eq!(logical_frame_color(&frame, 2, EPD_HEIGHT - 2), Color::Green);
    }

    #[test]
    fn rejects_wrong_output_frame_length() {
        let photo = solid_bmp(EPD_WIDTH, EPD_HEIGHT, Color::White);
        let mut frame = vec![0u8; EPD_FRAME_BYTES - 1];

        let result = render_epd_packed_frame_from_bmps_into(
            &mut frame,
            &PackedFrameRenderInput::new(&photo),
        );

        assert_eq!(result, Err(RenderError::InvalidFrameLength));
    }

    #[test]
    fn keeps_status_sprite_reserved_without_composing_it() {
        let photo = solid_bmp(EPD_WIDTH, EPD_HEIGHT, Color::White);
        let status = solid_bmp(2, 2, Color::Black);

        let frame = render_epd_packed_frame_from_bmps(
            &PackedFrameRenderInput::new(&photo).with_sprites(SpriteBmps {
                caption: None,
                date: None,
                notice: None,
                status: Some(&status),
            }),
        )
        .unwrap();

        assert_eq!(logical_frame_color(&frame, EPD_WIDTH - 1, 0), Color::White);
    }

    fn logical_frame_color(frame: &[u8], x: usize, y: usize) -> Color {
        let panel_x = EPD_WIDTH - 1 - x;
        let panel_y = EPD_HEIGHT - 1 - y;
        let byte = frame[panel_y * EPD_ROW_BYTES + panel_x / 2];
        let code = if panel_x.is_multiple_of(2) {
            byte >> 4
        } else {
            byte & 0x0F
        };

        match code {
            code if code == epd_color_code(Color::Black) => Color::Black,
            code if code == epd_color_code(Color::White) => Color::White,
            code if code == epd_color_code(Color::Yellow) => Color::Yellow,
            code if code == epd_color_code(Color::Red) => Color::Red,
            code if code == epd_color_code(Color::Blue) => Color::Blue,
            code if code == epd_color_code(Color::Green) => Color::Green,
            _ => panic!("unknown epd color code: {code}"),
        }
    }

    fn solid_bmp(width: usize, height: usize, color: Color) -> Vec<u8> {
        bmp_from_pixels(width, height, &vec![color; width * height])
    }

    fn bmp_from_pixels(width: usize, height: usize, pixels: &[Color]) -> Vec<u8> {
        assert_eq!(pixels.len(), width * height);

        let pixel_offset = 54usize;
        let row_stride = bmp_row_stride(width);
        let file_size = pixel_offset + row_stride * height;
        let mut bmp = vec![0u8; file_size];
        bmp[0..2].copy_from_slice(b"BM");
        bmp[2..6].copy_from_slice(&(file_size as u32).to_le_bytes());
        bmp[10..14].copy_from_slice(&(pixel_offset as u32).to_le_bytes());
        bmp[14..18].copy_from_slice(&40u32.to_le_bytes());
        bmp[18..22].copy_from_slice(&(width as i32).to_le_bytes());
        bmp[22..26].copy_from_slice(&(-(height as i32)).to_le_bytes());
        bmp[26..28].copy_from_slice(&1u16.to_le_bytes());
        bmp[28..30].copy_from_slice(&24u16.to_le_bytes());

        for y in 0..height {
            let row_offset = pixel_offset + y * row_stride;
            for x in 0..width {
                let pixel_offset = row_offset + x * 3;
                let (red, green, blue) = rgb(pixels[y * width + x]);
                bmp[pixel_offset..pixel_offset + 3].copy_from_slice(&[blue, green, red]);
            }
        }

        bmp
    }

    const fn rgb(color: Color) -> (u8, u8, u8) {
        match color {
            Color::White => (255, 255, 255),
            Color::Black => (0, 0, 0),
            Color::Yellow => (255, 255, 0),
            Color::Red => (255, 0, 0),
            Color::Blue => (0, 0, 255),
            Color::Green => (0, 255, 0),
        }
    }
}
