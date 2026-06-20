use super::{OverlaySlot, RenderError, RenderResource};
use crate::bmp::BmpImage;
use crate::epd::{pack_epd_pixels, set_logical_packed_frame_pixel, EPD_FRAME_BYTES};
use crate::screen::{Color, SCREEN_HEIGHT, SCREEN_WIDTH};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SpriteBmps<'a> {
    pub caption: Option<&'a [u8]>,
    pub date: Option<&'a [u8]>,
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

    let photo = BmpImage::parse(input.photo_bmp)
        .map_err(|error| RenderError::ResourceBmp(RenderResource::Photo, error))?;
    if photo.width() != SCREEN_WIDTH || photo.height() != SCREEN_HEIGHT {
        return Err(RenderError::InvalidPhotoSize);
    }

    draw_bmp_at(frame, &photo, 0, 0)?;

    if let Some(caption) = input.sprites.caption {
        draw_sprite_bmp(
            frame,
            RenderResource::Caption,
            caption,
            OverlaySlot::BottomLeft,
            input.placement,
        )?;
    }

    if let Some(date) = input.sprites.date {
        draw_sprite_bmp(
            frame,
            RenderResource::Date,
            date,
            OverlaySlot::BottomRight,
            input.placement,
        )?;
    }

    Ok(())
}

fn draw_sprite_bmp(
    frame: &mut [u8],
    resource: RenderResource,
    sprite_bmp: &[u8],
    slot: OverlaySlot,
    placement: SpritePlacement,
) -> Result<(), RenderError> {
    let sprite =
        BmpImage::parse(sprite_bmp).map_err(|error| RenderError::ResourceBmp(resource, error))?;
    let (x, y) = packed_overlay_origin(slot, sprite.width(), sprite.height(), placement);
    draw_sprite_bmp_at(frame, &sprite, x, y)
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

fn draw_sprite_bmp_at(
    frame: &mut [u8],
    image: &BmpImage<'_>,
    x: usize,
    y: usize,
) -> Result<(), RenderError> {
    let background_rgb = image.rgb_at(0, 0)?;

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

            let (red, green, blue) = image.rgb_at(source_x, source_y)?;
            if (red, green, blue) == background_rgb {
                continue;
            }
            let Some(color) = crate::bmp::exact_panel_color(red, green, blue) else {
                return Err(RenderError::Bmp(crate::bmp::BmpError::UnsupportedColor));
            };
            if !set_logical_packed_frame_pixel(frame, target_x, target_y, color) {
                return Err(RenderError::InvalidFrameLength);
            }
        }
    }

    Ok(())
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
