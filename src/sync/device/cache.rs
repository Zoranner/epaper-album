use crate::graphics::bmp::BmpImage;
use crate::storage::{ResourceStore, StorageBinaryRead};

pub(super) fn cached_image_is_renderable(store: &impl ResourceStore, sha256: &str) -> bool {
    let StorageBinaryRead::Bytes(bytes) = store.read_image_bytes(sha256) else {
        return false;
    };

    BmpImage::parse(&bytes)
        .map(|image| {
            image.width() == crate::screen::SCREEN_WIDTH
                && image.height() == crate::screen::SCREEN_HEIGHT
        })
        .unwrap_or(false)
}
