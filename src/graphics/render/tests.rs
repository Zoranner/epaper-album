use super::*;
use crate::bmp::bmp_row_stride;
use crate::display::Color;
use crate::epd::{epd_color_code, EPD_FRAME_BYTES, EPD_HEIGHT, EPD_ROW_BYTES, EPD_WIDTH};

#[test]
fn overlays_sprites_at_formal_slots() {
    let photo = solid_bmp(EPD_WIDTH, EPD_HEIGHT, Color::White);
    let caption = sprite_bmp(3, 2, Color::Black);
    let date = sprite_bmp(4, 2, Color::Red);

    let frame = render_epd_packed_frame_from_bmps(
        &PackedFrameRenderInput::new(&photo).with_sprites(SpriteBmps {
            caption: Some(&caption),
            date: Some(&date),
            status: None,
        }),
    )
    .unwrap();

    assert_eq!(logical_frame_color(&frame, 0, EPD_HEIGHT - 2), Color::White);
    assert_eq!(logical_frame_color(&frame, 1, EPD_HEIGHT - 2), Color::Black);
    assert_eq!(logical_frame_color(&frame, 2, EPD_HEIGHT - 1), Color::Black);
    assert_eq!(
        logical_frame_color(&frame, EPD_WIDTH - 4, EPD_HEIGHT - 2),
        Color::White
    );
    assert_eq!(
        logical_frame_color(&frame, EPD_WIDTH - 3, EPD_HEIGHT - 2),
        Color::Red
    );
    assert_eq!(
        logical_frame_color(&frame, EPD_WIDTH - 1, EPD_HEIGHT - 1),
        Color::Red
    );
    assert_eq!(logical_frame_color(&frame, 0, 0), Color::White);
}

#[test]
fn sprite_background_color_is_transparent_before_white_text_and_black_stroke() {
    let photo = solid_bmp(EPD_WIDTH, EPD_HEIGHT, Color::Blue);
    let caption = bmp_from_pixels(
        2,
        2,
        &[Color::Green, Color::Black, Color::White, Color::Green],
    );

    let frame = render_epd_packed_frame_from_bmps(
        &PackedFrameRenderInput::new(&photo).with_sprites(SpriteBmps {
            caption: Some(&caption),
            date: None,
            status: None,
        }),
    )
    .unwrap();

    assert_eq!(logical_frame_color(&frame, 0, EPD_HEIGHT - 2), Color::Blue);
    assert_eq!(logical_frame_color(&frame, 1, EPD_HEIGHT - 2), Color::Black);
    assert_eq!(logical_frame_color(&frame, 0, EPD_HEIGHT - 1), Color::White);
    assert_eq!(logical_frame_color(&frame, 1, EPD_HEIGHT - 1), Color::Blue);
}

#[test]
fn rejects_wrong_output_frame_length() {
    let photo = solid_bmp(EPD_WIDTH, EPD_HEIGHT, Color::White);
    let mut frame = vec![0u8; EPD_FRAME_BYTES - 1];

    let result =
        render_epd_packed_frame_from_bmps_into(&mut frame, &PackedFrameRenderInput::new(&photo));

    assert_eq!(result, Err(RenderError::InvalidFrameLength));
}

#[test]
fn render_error_names_invalid_sprite_resource() {
    let photo = solid_bmp(EPD_WIDTH, EPD_HEIGHT, Color::White);
    let invalid_caption = b"not-a-bmp";

    let result = render_epd_packed_frame_from_bmps(
        &PackedFrameRenderInput::new(&photo).with_sprites(SpriteBmps {
            caption: Some(invalid_caption),
            date: None,
            status: None,
        }),
    );

    assert_eq!(
        result.unwrap_err().to_string(),
        "caption-bmp-invalid-header"
    );
}

#[test]
fn keeps_status_sprite_reserved_without_composing_it() {
    let photo = solid_bmp(EPD_WIDTH, EPD_HEIGHT, Color::White);
    let status = solid_bmp(2, 2, Color::Black);

    let frame = render_epd_packed_frame_from_bmps(
        &PackedFrameRenderInput::new(&photo).with_sprites(SpriteBmps {
            caption: None,
            date: None,
            status: Some(&status),
        }),
    )
    .unwrap();

    assert_eq!(logical_frame_color(&frame, EPD_WIDTH - 1, 0), Color::White);
}

#[test]
fn builtin_error_page_renders_full_packed_frame_with_title_ink() {
    let frame = render_builtin_error_page_packed_frame(
        &BuiltinErrorPageInput::new("WIFI ERROR", "CANNOT CONNECT")
            .with_hint("CHECK WIFI SETTINGS")
            .with_detail("WIFI connect-failed"),
    );

    assert_eq!(frame.len(), EPD_FRAME_BYTES);
    assert!(has_non_white_pixel_in_title_area(&frame));
}

#[test]
fn builtin_error_page_separates_message_hint_and_detail_regions() {
    let frame = render_builtin_error_page_packed_frame(
        &BuiltinErrorPageInput::new("SYNC FAILED", "PHOTO UPDATE DID NOT COMPLETE")
            .with_hint("KEEPING CURRENT PHOTO")
            .with_detail("DETAIL SYNC HTTP 500 RETRY NEXT WAKE"),
    );

    assert!(has_non_white_pixel_in_region(&frame, 0..EPD_WIDTH, 48..148));
    assert!(has_non_white_pixel_in_region(&frame, 120..680, 170..230));
    assert!(has_non_white_pixel_in_region(&frame, 120..680, 264..308));
    assert!(has_non_white_pixel_in_region(&frame, 120..680, 356..430));
    assert!(has_non_white_pixel_in_region(&frame, 96..704, 330..334));
}

#[test]
fn builtin_error_page_title_rules_align_with_title_box() {
    let frame = render_builtin_error_page_packed_frame(
        &BuiltinErrorPageInput::new("SYNC ERROR", "PLAN SYNC FAILED")
            .with_hint("CHECK WIFI BASE URL AND SERVER"),
    );

    assert_eq!(black_row_bounds(&frame, 34), Some(56..744));
    assert_eq!(black_row_bounds(&frame, 42), Some(56..744));
    assert_eq!(black_row_bounds(&frame, 138), Some(56..744));
}

#[test]
fn builtin_error_page_centers_text_blocks_in_their_pixel_regions() {
    let frame = render_builtin_error_page_packed_frame(
        &BuiltinErrorPageInput::new("SYNC ERROR", "PLAN SYNC FAILED")
            .with_hint("CHECK WIFI BASE URL AND SERVER")
            .with_detail("resource.cloud.http-request"),
    );

    assert_color_region_is_vertically_centered(&frame, 56..744, 42..134, Color::White, 1);
    assert_color_region_is_vertically_centered(&frame, 96..704, 154..248, Color::Black, 1);
    assert_color_region_is_vertically_centered(&frame, 96..704, 250..330, Color::Black, 1);
    assert_color_region_is_vertically_centered(&frame, 96..704, 334..436, Color::Black, 1);
}

#[test]
fn placeholder_font_renders_underscore_as_baseline_not_unknown_box() {
    assert_eq!(
        glyph_pattern('_'),
        [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1F]
    );
    assert_ne!(glyph_pattern('_'), glyph_pattern('?'));
}

#[test]
fn placeholder_font_renders_error_detail_punctuation_without_unknown_boxes() {
    for character in ['(', ')', '='] {
        assert_ne!(glyph_pattern(character), glyph_pattern('?'));
    }
}

#[test]
fn builtin_error_page_wraps_long_detail_inside_detail_region() {
    let frame = render_builtin_error_page_packed_frame(
        &BuiltinErrorPageInput::new("SYNC ERROR", "PLAN SYNC FAILED")
            .with_hint("CHECK WIFI BASE URL AND SERVER")
            .with_detail(
                "cloud: api-status-500: ERROR CODE: SERVER TIMEOUT WHILE FETCHING PLAN DATA",
            ),
    );

    assert!(
        text_row_runs(&frame, 96..704, 334..436, Color::Black).len() >= 2,
        "long detail should wrap to multiple visible lines"
    );
}

#[test]
fn builtin_error_page_hint_and_detail_are_optional() {
    let frame = render_builtin_error_page_packed_frame(&BuiltinErrorPageInput::new(
        "NO PHOTO",
        "WAITING FOR IMAGE",
    ));

    assert_eq!(frame.len(), EPD_FRAME_BYTES);
    assert!(has_non_white_pixel_in_title_area(&frame));
}

fn has_non_white_pixel_in_title_area(frame: &[u8]) -> bool {
    has_non_white_pixel_in_region(frame, 0..EPD_WIDTH, 46..160)
}

fn has_non_white_pixel_in_region(
    frame: &[u8],
    xs: std::ops::Range<usize>,
    ys: std::ops::Range<usize>,
) -> bool {
    ys.clone().any(|y| {
        xs.clone()
            .any(|x| logical_frame_color(frame, x, y) != Color::White)
    })
}

fn assert_color_region_is_vertically_centered(
    frame: &[u8],
    xs: std::ops::Range<usize>,
    ys: std::ops::Range<usize>,
    color: Color,
    tolerance: usize,
) {
    let ink_rows = ys
        .clone()
        .filter(|y| {
            xs.clone()
                .any(|x| logical_frame_color(frame, x, *y) == color)
        })
        .collect::<Vec<_>>();
    let top = *ink_rows.first().expect("region should contain ink");
    let bottom = *ink_rows.last().expect("region should contain ink");
    let top_padding = top.saturating_sub(ys.start);
    let bottom_padding = ys.end.saturating_sub(1).saturating_sub(bottom);
    assert!(
        top_padding.abs_diff(bottom_padding) <= tolerance,
        "ink is not centered: top_padding={top_padding} bottom_padding={bottom_padding}"
    );
}

fn black_row_bounds(frame: &[u8], y: usize) -> Option<std::ops::Range<usize>> {
    let xs = (0..EPD_WIDTH)
        .filter(|x| logical_frame_color(frame, *x, y) == Color::Black)
        .collect::<Vec<_>>();
    Some(*xs.first()?..xs.last()?.saturating_add(1))
}

fn text_row_runs(
    frame: &[u8],
    xs: std::ops::Range<usize>,
    ys: std::ops::Range<usize>,
    color: Color,
) -> Vec<std::ops::Range<usize>> {
    let rows = ys
        .clone()
        .filter(|y| {
            xs.clone()
                .any(|x| logical_frame_color(frame, x, *y) == color)
        })
        .collect::<Vec<_>>();
    let mut runs = Vec::new();
    let Some(mut start) = rows.first().copied() else {
        return runs;
    };
    let mut previous = start;
    for row in rows.into_iter().skip(1) {
        if row > previous + 1 {
            runs.push(start..previous + 1);
            start = row;
        }
        previous = row;
    }
    runs.push(start..previous + 1);
    runs
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

fn sprite_bmp(width: usize, height: usize, color: Color) -> Vec<u8> {
    let mut pixels = vec![color; width * height];
    pixels[0] = Color::Green;
    bmp_from_pixels(width, height, &pixels)
}

fn bmp_from_pixels(width: usize, height: usize, pixels: &[Color]) -> Vec<u8> {
    let rgb_pixels = pixels.iter().copied().map(rgb).collect::<Vec<_>>();
    bmp_from_rgb_pixels(width, height, &rgb_pixels)
}

fn bmp_from_rgb_pixels(width: usize, height: usize, pixels: &[(u8, u8, u8)]) -> Vec<u8> {
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
            let (red, green, blue) = pixels[y * width + x];
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
