use super::text::{
    draw_centered_text_in_area, draw_centered_wrapped_text_in_area, fill_packed_frame_rect,
    TextStyle,
};
use crate::epd::{pack_epd_pixels, EPD_FRAME_BYTES};
use crate::screen::{Color, SCREEN_WIDTH};

const ERROR_TITLE_BOX_X: usize = 56;
const ERROR_TITLE_BOX_Y: usize = 42;
const ERROR_TITLE_BOX_HEIGHT: usize = 92;
const ERROR_MESSAGE_AREA_Y: usize = 154;
const ERROR_MESSAGE_AREA_HEIGHT: usize = 94;
const ERROR_HINT_AREA_Y: usize = 250;
const ERROR_HINT_AREA_HEIGHT: usize = 80;
const ERROR_DETAIL_AREA_X: usize = 96;
const ERROR_DETAIL_AREA_Y: usize = 334;
const ERROR_DETAIL_AREA_WIDTH: usize = SCREEN_WIDTH - 192;
const ERROR_DETAIL_AREA_HEIGHT: usize = 102;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BuiltinErrorPageInput<'a> {
    pub title: &'a str,
    pub message: &'a str,
    pub hint: &'a str,
    pub detail: &'a str,
}

impl<'a> BuiltinErrorPageInput<'a> {
    pub const fn new(title: &'a str, message: &'a str) -> Self {
        Self {
            title,
            message,
            hint: "",
            detail: "",
        }
    }

    pub const fn with_hint(mut self, hint: &'a str) -> Self {
        self.hint = hint;
        self
    }

    pub const fn with_detail(mut self, detail: &'a str) -> Self {
        self.detail = detail;
        self
    }
}

pub fn render_builtin_error_page_packed_frame(input: &BuiltinErrorPageInput<'_>) -> Vec<u8> {
    let mut frame = vec![pack_epd_pixels(Color::White, Color::White); EPD_FRAME_BYTES];
    render_builtin_error_page_into_frame(&mut frame, input);
    frame
}

fn render_builtin_error_page_into_frame(frame: &mut [u8], input: &BuiltinErrorPageInput<'_>) {
    let title_style = TextStyle {
        foreground: Color::White,
        background: Color::Black,
        padding_x: 20,
        padding_y: 12,
        margin_x: 0,
        margin_y: 0,
        glyph_width: 15,
        glyph_height: 21,
        glyph_gap: 4,
    };
    let message_style = TextStyle {
        foreground: Color::Black,
        background: Color::White,
        padding_x: 0,
        padding_y: 0,
        margin_x: 0,
        margin_y: 0,
        glyph_width: 10,
        glyph_height: 14,
        glyph_gap: 3,
    };
    let hint_style = TextStyle {
        glyph_width: 8,
        glyph_height: 11,
        glyph_gap: 2,
        ..message_style
    };
    let detail_style = TextStyle {
        foreground: Color::Black,
        background: Color::White,
        padding_x: 12,
        padding_y: 10,
        margin_x: 0,
        margin_y: 0,
        glyph_width: 8,
        glyph_height: 11,
        glyph_gap: 2,
    };

    fill_packed_frame_rect(
        frame,
        ERROR_TITLE_BOX_X,
        ERROR_TITLE_BOX_Y,
        SCREEN_WIDTH.saturating_sub(112),
        ERROR_TITLE_BOX_HEIGHT,
        Color::Black,
    );
    fill_packed_frame_rect(
        frame,
        56,
        138,
        SCREEN_WIDTH.saturating_sub(112),
        4,
        Color::Black,
    );
    fill_packed_frame_rect(
        frame,
        ERROR_TITLE_BOX_X,
        34,
        SCREEN_WIDTH.saturating_sub(112),
        4,
        Color::Black,
    );
    draw_centered_text_in_area(
        frame,
        ERROR_TITLE_BOX_Y,
        ERROR_TITLE_BOX_HEIGHT,
        input.title,
        &title_style,
    );

    fill_packed_frame_rect(
        frame,
        96,
        248,
        SCREEN_WIDTH.saturating_sub(192),
        2,
        Color::Black,
    );
    fill_packed_frame_rect(
        frame,
        96,
        330,
        SCREEN_WIDTH.saturating_sub(192),
        2,
        Color::Black,
    );
    fill_packed_frame_rect(
        frame,
        96,
        436,
        SCREEN_WIDTH.saturating_sub(192),
        2,
        Color::Black,
    );

    draw_centered_text_in_area(
        frame,
        ERROR_MESSAGE_AREA_Y,
        ERROR_MESSAGE_AREA_HEIGHT,
        input.message,
        &message_style,
    );

    if !input.hint.is_empty() {
        draw_centered_text_in_area(
            frame,
            ERROR_HINT_AREA_Y,
            ERROR_HINT_AREA_HEIGHT,
            input.hint,
            &hint_style,
        );
    }

    if !input.detail.is_empty() {
        draw_centered_wrapped_text_in_area(
            frame,
            ERROR_DETAIL_AREA_X,
            ERROR_DETAIL_AREA_Y,
            ERROR_DETAIL_AREA_WIDTH,
            ERROR_DETAIL_AREA_HEIGHT,
            input.detail,
            &detail_style,
        );
    }
}
