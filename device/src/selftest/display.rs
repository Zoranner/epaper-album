use crate::epd::{
    espidf::EspEpdBus, pack_epd_pixels, run_epd_packed_frame, set_logical_packed_frame_pixel,
    EPD_FRAME_BYTES,
};
use crate::render::{glyph_pattern, TextStyle};
use crate::screen::{Color, SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::selftest::hardware::{EpdProbe, HardwareSelfTestReport};
use crate::selftest::page::{
    self_test_bar_color_for_x, self_test_page_columns, self_test_page_subtitle, SelfTestPageModel,
    SelfTestPageSection, SELF_TEST_BODY_Y, SELF_TEST_LEFT_COLUMN_X, SELF_TEST_LINE_STEP_Y,
    SELF_TEST_PANEL_BORDER, SELF_TEST_PANEL_HEIGHT, SELF_TEST_PANEL_WIDTH, SELF_TEST_PANEL_X,
    SELF_TEST_PANEL_Y, SELF_TEST_RIGHT_COLUMN_X, SELF_TEST_SECTION_GAP_Y, SELF_TEST_SUBTITLE_Y,
    SELF_TEST_TITLE_STEP_Y, SELF_TEST_TITLE_Y,
};

pub fn refresh_epd_from_self_test_report(
    bus: &mut EspEpdBus,
    report: &HardwareSelfTestReport,
) -> EpdProbe {
    let mut frame = vec![pack_epd_pixels(Color::White, Color::White); EPD_FRAME_BYTES];
    draw_self_test_frame(&mut frame, report);

    match run_epd_packed_frame(bus, &frame) {
        Ok(()) => EpdProbe::Refreshed,
        Err(error) => epd_error_probe(error),
    }
}

fn draw_self_test_frame(frame: &mut [u8], report: &HardwareSelfTestReport) {
    draw_color_bars(frame);
    draw_panel(frame);

    let header_style = TextStyle {
        foreground: Color::Black,
        background: Color::White,
        padding_x: 0,
        padding_y: 0,
        margin_x: 0,
        margin_y: 0,
        glyph_width: 16,
        glyph_height: 24,
        glyph_gap: 3,
    };
    let section_style = TextStyle {
        glyph_width: 8,
        glyph_height: 13,
        glyph_gap: 2,
        ..header_style
    };
    let body_style = TextStyle {
        glyph_width: 7,
        glyph_height: 11,
        glyph_gap: 2,
        ..header_style
    };
    let subtitle_style = TextStyle {
        glyph_width: 7,
        glyph_height: 11,
        glyph_gap: 2,
        ..header_style
    };
    let model = SelfTestPageModel::from(report);

    draw_centered_text_on_frame(
        frame,
        SELF_TEST_PANEL_X,
        SELF_TEST_PANEL_WIDTH,
        SELF_TEST_TITLE_Y,
        "INKFRAME SELF TEST",
        &header_style,
    );
    draw_centered_text_on_frame(
        frame,
        SELF_TEST_PANEL_X,
        SELF_TEST_PANEL_WIDTH,
        SELF_TEST_SUBTITLE_Y,
        &self_test_page_subtitle(&model),
        &subtitle_style,
    );

    let columns = self_test_page_columns(&model);
    for (column_index, sections) in columns.iter().enumerate() {
        let x = if column_index == 0 {
            SELF_TEST_LEFT_COLUMN_X
        } else {
            SELF_TEST_RIGHT_COLUMN_X
        };
        let mut y = SELF_TEST_BODY_Y;
        for section in sections {
            y = draw_self_test_section(frame, x, y, section, &section_style, &body_style);
        }
    }
}

fn draw_color_bars(frame: &mut [u8]) {
    for y in 0..SCREEN_HEIGHT {
        for x in 0..SCREEN_WIDTH {
            set_logical_packed_frame_pixel(frame, x, y, self_test_bar_color_for_x(x));
        }
    }
}

fn draw_self_test_section(
    frame: &mut [u8],
    x: usize,
    mut y: usize,
    section: &SelfTestPageSection,
    section_style: &TextStyle,
    body_style: &TextStyle,
) -> usize {
    draw_text_on_frame(frame, x, y, section.title, section_style);
    y += SELF_TEST_TITLE_STEP_Y;

    for line in &section.lines {
        draw_text_on_frame(frame, x, y, line, body_style);
        y += SELF_TEST_LINE_STEP_Y;
    }

    y + SELF_TEST_SECTION_GAP_Y
}

fn draw_panel(frame: &mut [u8]) {
    fill_frame_rect(
        frame,
        SELF_TEST_PANEL_X,
        SELF_TEST_PANEL_Y,
        SELF_TEST_PANEL_WIDTH,
        SELF_TEST_PANEL_HEIGHT,
        Color::Black,
    );
    fill_frame_rect(
        frame,
        SELF_TEST_PANEL_X + SELF_TEST_PANEL_BORDER,
        SELF_TEST_PANEL_Y + SELF_TEST_PANEL_BORDER,
        SELF_TEST_PANEL_WIDTH - SELF_TEST_PANEL_BORDER * 2,
        SELF_TEST_PANEL_HEIGHT - SELF_TEST_PANEL_BORDER * 2,
        Color::White,
    );
}

fn draw_text_on_frame(frame: &mut [u8], x: usize, y: usize, text: &str, style: &TextStyle) {
    let mut cursor_x = x.saturating_add(style.padding_x);
    let glyph_y = y.saturating_add(style.padding_y);
    for character in text.chars() {
        if character.is_whitespace() {
            cursor_x = cursor_x
                .saturating_add(style.glyph_width)
                .saturating_add(style.glyph_gap);
            continue;
        }

        draw_glyph_on_frame(
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

fn draw_centered_text_on_frame(
    frame: &mut [u8],
    area_x: usize,
    area_width: usize,
    y: usize,
    text: &str,
    style: &TextStyle,
) {
    let text_width = text_pixel_width(text, style);
    let x = area_x + area_width.saturating_sub(text_width) / 2;
    draw_text_on_frame(frame, x, y, text, style);
}

fn text_pixel_width(text: &str, style: &TextStyle) -> usize {
    let glyph_count = text.chars().count();
    if glyph_count == 0 {
        return 0;
    }

    glyph_count
        .saturating_mul(style.glyph_width)
        .saturating_add(
            glyph_count
                .saturating_sub(1)
                .saturating_mul(style.glyph_gap),
        )
}

fn draw_glyph_on_frame(
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
        let source_y = glyph_y * pattern.len() / height;
        let row = pattern[source_y];
        for glyph_x in 0..width {
            let source_x = glyph_x * 5 / width;
            let bit = 1 << (4 - source_x);
            if row & bit != 0 {
                set_logical_packed_frame_pixel(frame, x + glyph_x, y + glyph_y, color);
            }
        }
    }
}

fn fill_frame_rect(
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

fn epd_error_probe(error: crate::epd::EpdError) -> EpdProbe {
    match error {
        crate::epd::EpdError::BusyTimeout => EpdProbe::BusyTimeout,
        crate::epd::EpdError::Transport => EpdProbe::TransportError,
    }
}
