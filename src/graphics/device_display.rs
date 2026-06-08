use crate::cloud::sprite_sha256;
use crate::device_runtime::{DeviceDisplay, DisplayRefreshRequest};
use crate::epd::{run_epd_packed_frame, EpdBus, EpdError};
use crate::render::{
    render_epd_packed_frame_from_bmps, PackedFrameRenderInput, RenderError, SpriteBmps,
    SpritePlacement,
};
use crate::storage::{image_bmp_path, read_binary_file, sprite_bmp_path, StorageBinaryRead};
use std::fmt;

const OVERLAY_MARGIN: usize = 18;

pub trait DisplayResourceReader {
    type Error: fmt::Display;

    fn read_photo_bmp(&mut self, sha256: &str) -> Result<Vec<u8>, Self::Error>;
    fn read_sprite_bmp(&mut self, key: &str) -> Result<Option<Vec<u8>>, Self::Error>;
}

#[derive(Debug, Default)]
pub struct SdCardDisplayResourceReader;

#[derive(Debug, Default)]
pub struct MountedSdCardDisplayResourceReader;

impl DisplayResourceReader for SdCardDisplayResourceReader {
    type Error = DisplayReadError;

    fn read_photo_bmp(&mut self, sha256: &str) -> Result<Vec<u8>, Self::Error> {
        match read_binary_file(image_bmp_path(sha256)) {
            StorageBinaryRead::Bytes(bytes) => Ok(bytes),
            StorageBinaryRead::Missing => Err(DisplayReadError::MissingPhoto),
            StorageBinaryRead::FormatError => Err(DisplayReadError::ReadError),
            StorageBinaryRead::MountError => Err(DisplayReadError::MountError),
            StorageBinaryRead::ReadError => Err(DisplayReadError::ReadError),
        }
    }

    fn read_sprite_bmp(&mut self, key: &str) -> Result<Option<Vec<u8>>, Self::Error> {
        match read_binary_file(sprite_bmp_path(key)) {
            StorageBinaryRead::Bytes(bytes) => Ok(Some(bytes)),
            StorageBinaryRead::Missing => Ok(None),
            StorageBinaryRead::FormatError => Err(DisplayReadError::ReadError),
            StorageBinaryRead::MountError => Err(DisplayReadError::MountError),
            StorageBinaryRead::ReadError => Err(DisplayReadError::ReadError),
        }
    }
}

impl DisplayResourceReader for MountedSdCardDisplayResourceReader {
    type Error = DisplayReadError;

    fn read_photo_bmp(&mut self, sha256: &str) -> Result<Vec<u8>, Self::Error> {
        match crate::storage::read_binary_file_mounted(image_bmp_path(sha256)) {
            StorageBinaryRead::Bytes(bytes) => Ok(bytes),
            StorageBinaryRead::Missing => Err(DisplayReadError::MissingPhoto),
            StorageBinaryRead::FormatError => Err(DisplayReadError::ReadError),
            StorageBinaryRead::MountError => Err(DisplayReadError::MountError),
            StorageBinaryRead::ReadError => Err(DisplayReadError::ReadError),
        }
    }

    fn read_sprite_bmp(&mut self, key: &str) -> Result<Option<Vec<u8>>, Self::Error> {
        match crate::storage::read_binary_file_mounted(sprite_bmp_path(key)) {
            StorageBinaryRead::Bytes(bytes) => Ok(Some(bytes)),
            StorageBinaryRead::Missing => Ok(None),
            StorageBinaryRead::FormatError => Err(DisplayReadError::ReadError),
            StorageBinaryRead::MountError => Err(DisplayReadError::MountError),
            StorageBinaryRead::ReadError => Err(DisplayReadError::ReadError),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayReadError {
    MissingPhoto,
    MountError,
    ReadError,
}

impl fmt::Display for DisplayReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPhoto => formatter.write_str("missing-photo"),
            Self::MountError => formatter.write_str("mount-error"),
            Self::ReadError => formatter.write_str("read-error"),
        }
    }
}

impl std::error::Error for DisplayReadError {}

pub struct PackedFrameDisplay<R, B> {
    reader: R,
    bus: B,
}

impl<R, B> PackedFrameDisplay<R, B> {
    pub const fn new(reader: R, bus: B) -> Self {
        Self { reader, bus }
    }

    pub fn into_parts(self) -> (R, B) {
        (self.reader, self.bus)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceDisplayError<R> {
    Read(R),
    Render(RenderError),
    Epd(EpdError),
}

impl<R> fmt::Display for DeviceDisplayError<R>
where
    R: fmt::Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(formatter, "display-read: {error}"),
            Self::Render(error) => write!(formatter, "render: {error}"),
            Self::Epd(error) => write!(formatter, "epd: {error}"),
        }
    }
}

impl<R> std::error::Error for DeviceDisplayError<R> where R: fmt::Debug + fmt::Display {}

impl<R, B> DeviceDisplay for PackedFrameDisplay<R, B>
where
    R: DisplayResourceReader,
    B: EpdBus,
{
    type Error = DeviceDisplayError<R::Error>;

    fn refresh(&mut self, request: DisplayRefreshRequest) -> Result<(), Self::Error> {
        let photo = self
            .reader
            .read_photo_bmp(&request.item.image_sha256)
            .map_err(DeviceDisplayError::Read)?;
        let caption =
            read_caption_sprite(&mut self.reader, &request).map_err(DeviceDisplayError::Read)?;
        let date =
            read_date_sprite(&mut self.reader, &request).map_err(DeviceDisplayError::Read)?;
        let notice =
            read_notice_sprite(&mut self.reader, &request).map_err(DeviceDisplayError::Read)?;

        let frame = render_epd_packed_frame_from_bmps(
            &PackedFrameRenderInput::new(&photo)
                .with_sprites(SpriteBmps {
                    caption: caption.as_deref(),
                    date: date.as_deref(),
                    notice: notice.as_deref(),
                    status: None,
                })
                .with_placement(SpritePlacement {
                    margin_x: OVERLAY_MARGIN,
                    margin_y: OVERLAY_MARGIN,
                }),
        )
        .map_err(DeviceDisplayError::Render)?;

        run_epd_packed_frame(&mut self.bus, &frame).map_err(DeviceDisplayError::Epd)
    }
}

fn read_caption_sprite<R>(
    reader: &mut R,
    request: &DisplayRefreshRequest,
) -> Result<Option<Vec<u8>>, R::Error>
where
    R: DisplayResourceReader,
{
    if request.item.caption.trim().is_empty() {
        return Ok(None);
    }

    let sha256 = sprite_sha256("caption", &request.item.caption);
    reader.read_sprite_bmp(&sha256)
}

fn read_date_sprite<R>(
    reader: &mut R,
    request: &DisplayRefreshRequest,
) -> Result<Option<Vec<u8>>, R::Error>
where
    R: DisplayResourceReader,
{
    let Some(date) = request.display_state.date else {
        return Ok(None);
    };

    let sha256 = sprite_sha256("date", &date.to_string());
    reader.read_sprite_bmp(&sha256)
}

fn read_notice_sprite<R>(
    reader: &mut R,
    request: &DisplayRefreshRequest,
) -> Result<Option<Vec<u8>>, R::Error>
where
    R: DisplayResourceReader,
{
    let Some(notice) = request.notice else {
        return Ok(None);
    };

    let sha256 = sprite_sha256("notice", notice.text());
    reader.read_sprite_bmp(&sha256)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bmp::bmp_row_stride;
    use crate::display::Color;
    use crate::epd::{EpdError, EPD_FRAME_BYTES, EPD_HEIGHT, EPD_ROW_BYTES, EPD_WIDTH};
    use crate::model::{DisplayItem, DisplayState, LocalDate};
    use crate::render::RenderNotice;
    use crate::state::RefreshReason;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct MockReader {
        photos: BTreeMap<String, Vec<u8>>,
        sprites: BTreeMap<String, Vec<u8>>,
    }

    impl DisplayResourceReader for MockReader {
        type Error = DisplayReadError;

        fn read_photo_bmp(&mut self, sha256: &str) -> Result<Vec<u8>, Self::Error> {
            self.photos
                .get(sha256)
                .cloned()
                .ok_or(DisplayReadError::MissingPhoto)
        }

        fn read_sprite_bmp(&mut self, key: &str) -> Result<Option<Vec<u8>>, Self::Error> {
            Ok(self.sprites.get(key).cloned())
        }
    }

    #[derive(Default)]
    struct MockBus {
        row_count: usize,
        frame_bytes: usize,
    }

    impl EpdBus for MockBus {
        fn reset(&mut self) -> Result<(), EpdError> {
            Ok(())
        }

        fn wait_until_ready(&mut self) -> Result<(), EpdError> {
            Ok(())
        }

        fn delay_ms(&mut self, _milliseconds: u32) {}

        fn command(&mut self, _command: u8) -> Result<(), EpdError> {
            Ok(())
        }

        fn data(&mut self, data: &[u8]) -> Result<(), EpdError> {
            if data.len() == EPD_ROW_BYTES {
                self.row_count += 1;
                self.frame_bytes += data.len();
            }
            Ok(())
        }
    }

    #[test]
    fn display_composes_cached_photo_and_sprites() {
        let mut reader = MockReader::default();
        reader.photos.insert(
            "photo".to_string(),
            solid_bmp(EPD_WIDTH, EPD_HEIGHT, Color::White),
        );
        reader.sprites.insert(
            sprite_sha256("caption", "caption"),
            solid_bmp(8, 4, Color::Black),
        );
        reader.sprites.insert(
            sprite_sha256("date", "2026-06-08"),
            solid_bmp(8, 4, Color::Red),
        );
        reader.sprites.insert(
            sprite_sha256("notice", RenderNotice::LowBattery.text()),
            solid_bmp(8, 4, Color::Blue),
        );
        let bus = MockBus::default();
        let mut display = PackedFrameDisplay::new(reader, bus);

        display
            .refresh(request(Some(RenderNotice::LowBattery)))
            .unwrap();
        let (_reader, bus) = display.into_parts();

        assert_eq!(bus.row_count, EPD_HEIGHT);
        assert_eq!(bus.frame_bytes, EPD_FRAME_BYTES);
    }

    #[test]
    fn display_allows_missing_sprites_for_photo_refresh() {
        let mut reader = MockReader::default();
        reader.photos.insert(
            "photo".to_string(),
            solid_bmp(EPD_WIDTH, EPD_HEIGHT, Color::White),
        );
        let bus = MockBus::default();
        let mut display = PackedFrameDisplay::new(reader, bus);

        display.refresh(request(None)).unwrap();
        let (_reader, bus) = display.into_parts();

        assert_eq!(bus.row_count, EPD_HEIGHT);
    }

    fn request(notice: Option<RenderNotice>) -> DisplayRefreshRequest {
        DisplayRefreshRequest {
            item: DisplayItem {
                plan_content_hash: Some("hash".to_string()),
                date: LocalDate::parse("2026-06-08").unwrap(),
                image_sha256: "photo".to_string(),
                caption: "caption".to_string(),
            },
            display_state: DisplayState {
                plan_content_hash: Some("hash".to_string()),
                date: Some(LocalDate::parse("2026-06-08").unwrap()),
                image_sha256: Some("photo".to_string()),
                caption: Some("caption".to_string()),
                refreshed_at_unix_secs: Some(100),
            },
            reason: RefreshReason::FirstBoot,
            notice,
            now_epoch_seconds: 100,
        }
    }

    fn solid_bmp(width: usize, height: usize, color: Color) -> Vec<u8> {
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

        let (red, green, blue) = rgb(color);
        for y in 0..height {
            let row_offset = pixel_offset + y * row_stride;
            for x in 0..width {
                let pixel_offset = row_offset + x * 3;
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
