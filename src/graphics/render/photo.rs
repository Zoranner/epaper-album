use super::text::{
    draw_text, OverlaySlot, TextStyle, GLYPH_GAP, PLACEHOLDER_GLYPH_HEIGHT, PLACEHOLDER_GLYPH_WIDTH,
};
use crate::display::{Color, ScreenBuffer, SCREEN_HEIGHT, SCREEN_WIDTH};

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
    pub style: TextStyle,
}

impl<'a> RenderInput<'a> {
    pub const fn new(caption: &'a str, date: &'a str) -> Self {
        Self {
            image: None,
            caption,
            date,
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
