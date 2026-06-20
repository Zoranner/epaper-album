pub mod error_page;
pub mod frame;
pub mod photo;
pub mod text;

pub use error_page::{render_builtin_error_page_packed_frame, BuiltinErrorPageInput};
pub use frame::{
    render_epd_packed_frame_from_bmps, render_epd_packed_frame_from_bmps_into,
    PackedFrameRenderInput, SpriteBmps, SpritePlacement,
};
pub use photo::{render_into, render_photo_page, RenderImage, RenderInput};
pub use text::{
    draw_placeholder_text, draw_solid_block, draw_text, draw_text_at, glyph_pattern, text_size,
    OverlaySlot, TextStyle,
};

use crate::bmp::BmpError;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderError {
    InvalidPhotoSize,
    InvalidFrameLength,
    Bmp(BmpError),
    ResourceBmp(RenderResource, BmpError),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPhotoSize => write!(f, "invalid-photo-size"),
            Self::InvalidFrameLength => write!(f, "invalid-frame-length"),
            Self::Bmp(error) => write!(f, "bmp-{error}"),
            Self::ResourceBmp(resource, error) => write!(f, "{}-bmp-{error}", resource.label()),
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
pub enum RenderResource {
    Photo,
    Caption,
    Date,
}

impl RenderResource {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Photo => "photo",
            Self::Caption => "caption",
            Self::Date => "date",
        }
    }
}

#[cfg(test)]
mod tests;
