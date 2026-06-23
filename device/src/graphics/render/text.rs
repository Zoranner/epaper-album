use crate::epd::set_logical_packed_frame_pixel;
use crate::screen::{Color, ScreenBuffer, SCREEN_HEIGHT, SCREEN_WIDTH};

pub(crate) const PLACEHOLDER_GLYPH_WIDTH: usize = 5;
pub(crate) const PLACEHOLDER_GLYPH_HEIGHT: usize = 7;
pub(crate) const GLYPH_GAP: usize = 1;

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

pub(crate) fn draw_centered_text_in_area(
    frame: &mut [u8],
    area_y: usize,
    area_height: usize,
    text: &str,
    style: &TextStyle,
) {
    let Some((block_width, block_height)) = text_size(text, style) else {
        return;
    };
    let x = SCREEN_WIDTH.saturating_sub(block_width) / 2;
    let y = area_y.saturating_add(area_height.saturating_sub(block_height) / 2);
    draw_text_on_packed_frame(frame, x, y, text, style);
}

pub(crate) fn draw_centered_wrapped_text_in_area(
    frame: &mut [u8],
    area_x: usize,
    area_y: usize,
    area_width: usize,
    area_height: usize,
    text: &str,
    style: &TextStyle,
) {
    let lines = wrap_text_lines(text, style, area_width);
    let Some((_, line_height)) = text_size("A", style) else {
        return;
    };
    let line_gap = 2usize;
    let max_lines = area_height
        .saturating_add(line_gap)
        .checked_div(line_height.saturating_add(line_gap))
        .unwrap_or(0);
    let line_count = lines.len().min(max_lines);
    if line_count == 0 {
        return;
    }

    let total_height = line_count
        .saturating_mul(line_height)
        .saturating_add(line_count.saturating_sub(1).saturating_mul(line_gap));
    let mut y = area_y.saturating_add(area_height.saturating_sub(total_height) / 2);
    for line in lines.iter().take(line_count) {
        let Some((line_width, _)) = text_size(line, style) else {
            continue;
        };
        let x = area_x.saturating_add(area_width.saturating_sub(line_width) / 2);
        draw_text_on_packed_frame(frame, x, y, line, style);
        y = y.saturating_add(line_height).saturating_add(line_gap);
    }
}

fn draw_text_on_packed_frame(frame: &mut [u8], x: usize, y: usize, text: &str, style: &TextStyle) {
    let Some((block_width, block_height)) = text_size(text, style) else {
        return;
    };

    fill_packed_frame_rect(frame, x, y, block_width, block_height, style.background);

    let mut cursor_x = x.saturating_add(style.padding_x);
    let glyph_y = y.saturating_add(style.padding_y);
    for character in text.chars() {
        if character.is_whitespace() {
            cursor_x = cursor_x
                .saturating_add(style.glyph_width)
                .saturating_add(style.glyph_gap);
            continue;
        }

        draw_glyph_on_packed_frame(
            frame,
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

fn draw_glyph_on_packed_frame(
    frame: &mut [u8],
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
                set_logical_packed_frame_pixel(frame, x + glyph_x, y + glyph_y, color);
            }
        }
    }
}

fn wrap_text_lines(text: &str, style: &TextStyle, max_width: usize) -> Vec<String> {
    let max_chars = max_chars_per_wrapped_line(style, max_width);
    if max_chars == 0 {
        return Vec::new();
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{current} {word}")
        };
        if text_fits_width(&candidate, style, max_width) {
            current = candidate;
            continue;
        }

        if !current.is_empty() {
            lines.push(current);
            current = String::new();
        }

        if text_fits_width(word, style, max_width) {
            current.push_str(word);
            continue;
        }

        let mut chunk = String::new();
        for character in word.chars() {
            chunk.push(character);
            if chunk.chars().count() == max_chars {
                lines.push(chunk);
                chunk = String::new();
            }
        }
        current = chunk;
    }

    if !current.is_empty() {
        lines.push(current);
    }

    lines
}

fn text_fits_width(text: &str, style: &TextStyle, max_width: usize) -> bool {
    text_size(text, style).is_some_and(|(width, _)| width <= max_width)
}

fn max_chars_per_wrapped_line(style: &TextStyle, max_width: usize) -> usize {
    let available_width = max_width.saturating_sub(style.padding_x.saturating_mul(2));
    if style.glyph_width == 0 || available_width < style.glyph_width {
        return 0;
    }

    available_width
        .saturating_sub(style.glyph_width)
        .checked_div(style.glyph_width.saturating_add(style.glyph_gap))
        .unwrap_or(0)
        .saturating_add(1)
}

pub(crate) fn fill_packed_frame_rect(
    frame: &mut [u8],
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    color: Color,
) {
    let end_x = x.saturating_add(width).min(SCREEN_WIDTH);
    let end_y = y.saturating_add(height).min(SCREEN_HEIGHT);

    for pixel_y in y..end_y {
        for pixel_x in x..end_x {
            set_logical_packed_frame_pixel(frame, pixel_x, pixel_y, color);
        }
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
        '(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        ')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        '%' => [0x19, 0x19, 0x02, 0x04, 0x08, 0x13, 0x13],
        '=' => [0x00, 0x00, 0x1F, 0x00, 0x1F, 0x00, 0x00],
        '_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1F],
        '?' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x00, 0x04],
        _ => [0x1F, 0x11, 0x15, 0x15, 0x15, 0x11, 0x1F],
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
