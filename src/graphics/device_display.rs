use crate::bmp::BmpImage;
use crate::device_runtime::{DeviceDisplay, DisplayRefreshRequest, ErrorRefreshRequest};
use crate::display::{SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::epd::{run_epd_packed_frame, EpdBus, EpdError};
use crate::render::{
    render_builtin_error_page_packed_frame, render_epd_packed_frame_from_bmps,
    BuiltinErrorPageInput, PackedFrameRenderInput, RenderError, SpriteBmps, SpritePlacement,
};
use crate::storage::{image_bmp_path, read_binary_file, sprite_bmp_path, StorageBinaryRead};
use std::fmt;

const OVERLAY_MARGIN: usize = 18;

pub trait DisplayResourceReader {
    type Error: fmt::Display;

    fn has_photo(&self, sha256: &str) -> bool;
    fn read_photo_bmp(&mut self, sha256: &str) -> Result<Vec<u8>, Self::Error>;
    fn read_sprite_bmp(&mut self, key: &str) -> Result<Option<Vec<u8>>, Self::Error>;
}

#[derive(Debug, Default)]
pub struct SdCardDisplayResourceReader;

#[derive(Debug, Default)]
pub struct MountedSdCardDisplayResourceReader;

impl DisplayResourceReader for SdCardDisplayResourceReader {
    type Error = DisplayReadError;

    fn has_photo(&self, sha256: &str) -> bool {
        photo_bmp_is_renderable(read_binary_file(image_bmp_path(sha256)))
    }

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

    fn has_photo(&self, sha256: &str) -> bool {
        photo_bmp_is_renderable(crate::storage::read_binary_file_mounted(image_bmp_path(
            sha256,
        )))
    }

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

fn photo_bmp_is_renderable(read: StorageBinaryRead) -> bool {
    let StorageBinaryRead::Bytes(bytes) = read else {
        return false;
    };

    BmpImage::parse(&bytes)
        .map(|image| image.width() == SCREEN_WIDTH && image.height() == SCREEN_HEIGHT)
        .unwrap_or(false)
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
            .read_photo_bmp(&request.plan.image)
            .map_err(DeviceDisplayError::Read)?;
        let caption =
            read_caption_sprite(&mut self.reader, &request).map_err(DeviceDisplayError::Read)?;
        let date =
            read_date_sprite(&mut self.reader, &request).map_err(DeviceDisplayError::Read)?;

        let frame = render_epd_packed_frame_from_bmps(
            &PackedFrameRenderInput::new(&photo)
                .with_sprites(SpriteBmps {
                    caption: caption.as_deref(),
                    date: date.as_deref(),
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

    fn refresh_error_page(&mut self, request: ErrorRefreshRequest) -> Result<(), Self::Error> {
        let frame = render_builtin_error_page_packed_frame(
            &BuiltinErrorPageInput::new(&request.title, &request.message)
                .with_hint(&request.hint)
                .with_detail(&request.detail),
        );

        run_epd_packed_frame(&mut self.bus, &frame).map_err(DeviceDisplayError::Epd)
    }

    fn has_image(&self, sha256: &str) -> bool {
        self.reader.has_photo(sha256)
    }
}

fn read_caption_sprite<R>(
    reader: &mut R,
    request: &DisplayRefreshRequest,
) -> Result<Option<Vec<u8>>, R::Error>
where
    R: DisplayResourceReader,
{
    if request.plan.caption.trim().is_empty() {
        return Ok(None);
    }

    let Some(sha256) = request.sprites.caption.as_deref() else {
        return Ok(None);
    };
    reader.read_sprite_bmp(sha256)
}

fn read_date_sprite<R>(
    reader: &mut R,
    request: &DisplayRefreshRequest,
) -> Result<Option<Vec<u8>>, R::Error>
where
    R: DisplayResourceReader,
{
    let Some(sha256) = request.sprites.date.as_deref() else {
        return Ok(None);
    };
    reader.read_sprite_bmp(sha256)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bmp::bmp_row_stride;
    use crate::display::Color;
    use crate::epd::{
        epd_color_code, EpdError, EPD_FRAME_BYTES, EPD_HEIGHT, EPD_ROW_BYTES, EPD_WIDTH,
    };
    use crate::model::{LocalDate, Plan};
    use crate::state::RefreshReason;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct MockReader {
        photos: BTreeMap<String, Vec<u8>>,
        sprites: BTreeMap<String, Vec<u8>>,
        photo_reads: usize,
        sprite_reads: usize,
    }

    impl DisplayResourceReader for MockReader {
        type Error = DisplayReadError;

        fn has_photo(&self, sha256: &str) -> bool {
            self.photos.get(sha256).is_some_and(|bytes| {
                BmpImage::parse(bytes)
                    .map(|image| image.width() == SCREEN_WIDTH && image.height() == SCREEN_HEIGHT)
                    .unwrap_or(false)
            })
        }

        fn read_photo_bmp(&mut self, sha256: &str) -> Result<Vec<u8>, Self::Error> {
            self.photo_reads += 1;
            self.photos
                .get(sha256)
                .cloned()
                .ok_or(DisplayReadError::MissingPhoto)
        }

        fn read_sprite_bmp(&mut self, key: &str) -> Result<Option<Vec<u8>>, Self::Error> {
            self.sprite_reads += 1;
            Ok(self.sprites.get(key).cloned())
        }
    }

    #[derive(Default)]
    struct MockBus {
        row_count: usize,
        frame_bytes: usize,
        frame: Vec<u8>,
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
                self.frame.extend_from_slice(data);
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
        reader
            .sprites
            .insert("caption-sha".to_string(), solid_bmp(8, 4, Color::Black));
        reader
            .sprites
            .insert("date-sha".to_string(), solid_bmp(8, 4, Color::Red));
        let bus = MockBus::default();
        let mut display = PackedFrameDisplay::new(reader, bus);

        display.refresh(request()).unwrap();
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

        display.refresh(request()).unwrap();
        let (_reader, bus) = display.into_parts();

        assert_eq!(bus.row_count, EPD_HEIGHT);
    }

    #[test]
    fn display_rejects_unrenderable_cached_photo() {
        let mut reader = MockReader::default();
        reader
            .photos
            .insert("photo".to_string(), b"not-a-bmp".to_vec());
        let bus = MockBus::default();
        let display = PackedFrameDisplay::new(reader, bus);

        assert!(!display.has_image("photo"));
    }

    #[test]
    fn display_error_page_renders_without_reading_resources() {
        let reader = MockReader::default();
        let bus = MockBus::default();
        let mut display = PackedFrameDisplay::new(reader, bus);

        display
            .refresh_error_page(ErrorRefreshRequest {
                title: "CONFIG ERROR".to_string(),
                message: "DEVICE CONFIG IS MISSING".to_string(),
                hint: "CHECK /SDCARD/CONFIG.TOML".to_string(),
                detail: "WIFI BASE URL AND SECRET KEY REQUIRED".to_string(),
                now_epoch_seconds: 100,
            })
            .unwrap();
        let (reader, bus) = display.into_parts();

        assert_eq!(reader.photo_reads, 0);
        assert_eq!(reader.sprite_reads, 0);
        assert_eq!(bus.row_count, EPD_HEIGHT);
        assert_eq!(bus.frame_bytes, EPD_FRAME_BYTES);
        assert!(has_non_white_pixel_in_region(
            &bus.frame,
            120..680,
            356..430
        ));
    }

    fn request() -> DisplayRefreshRequest {
        DisplayRefreshRequest {
            plan: Plan {
                date: LocalDate::parse("2026-06-08").unwrap(),
                image: "photo".to_string(),
                caption: "caption".to_string(),
            },
            date: LocalDate::parse("2026-06-08").unwrap(),
            reason: RefreshReason::FirstBoot,
            sprites: crate::device_runtime::SpriteSet {
                caption: Some("caption-sha".to_string()),
                date: Some("date-sha".to_string()),
            },
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
}
