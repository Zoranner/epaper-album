use crate::display::{Color, ScreenBuffer, SCREEN_HEIGHT, SCREEN_WIDTH};

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
