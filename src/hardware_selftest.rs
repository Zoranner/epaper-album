use crate::config::{Config, CONFIG_PATH};
use crate::display::{Color, SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::epd::{
    espidf::EspEpdBus, run_epd_hardware_self_test, run_epd_prepacked_frame, set_packed_frame_pixel,
    EPD_FRAME_BYTES, EPD_ROW_BYTES,
};
use crate::pmic::espidf::{chip_id_is_axp2101, init_axp2101_for_photo_painter};
use crate::power::espidf::WakeProbe;
use crate::render::{OverlaySlot, TextStyle};
use crate::selftest::{ConfigProbe, RenderProbe, SelfTestReport, StorageProbe};
use crate::storage::{with_mounted_sdcard_parts, StorageBinaryRead, StorageRead};
use crate::wifi::espidf::{probe_test_network, HttpProbe, WifiProbe};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const TEST_BMP_PATH: &str = "/sdcard/test.bmp";
const WAKE_TEST_MARKER_PATH: &str = "/sdcard/wake-test.txt";
const TITLE_TEXT_WIDTH: usize = 89;
const TITLE_TEXT_HEIGHT: usize = 20;
const TITLE_TEXT_BYTES_PER_ROW: usize = 12;
const TITLE_TEXT_BITMAP: [u8; 240] = [
    0x01, 0xC0, 0x00, 0x0E, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0xC0, 0x00, 0x0E,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0xC0, 0x00, 0x07, 0x00, 0x01, 0xFF, 0xF0,
    0x00, 0x00, 0x00, 0x00, 0x01, 0xC0, 0x07, 0xFF, 0xFE, 0x01, 0xFF, 0xF0, 0x00, 0x00, 0x06, 0x00,
    0xFF, 0xFF, 0xC7, 0xFF, 0xFE, 0x01, 0xFF, 0xF0, 0x00, 0x00, 0x0E, 0x00, 0xFF, 0xFF, 0xC0, 0xE0,
    0x70, 0x00, 0x0E, 0x00, 0x00, 0x00, 0x0E, 0x00, 0xE1, 0xC1, 0xC0, 0xE0, 0x70, 0x00, 0x0E, 0x00,
    0xF8, 0x1F, 0x3F, 0x80, 0xE1, 0xC1, 0xC0, 0xE0, 0xF0, 0x00, 0x0E, 0x01, 0xFC, 0x3F, 0x3F, 0x80,
    0xE1, 0xC1, 0xC0, 0x70, 0xE0, 0x00, 0x0E, 0x03, 0xFE, 0x7F, 0x3F, 0x80, 0xE1, 0xC1, 0xC0, 0x70,
    0xE0, 0x00, 0x0E, 0x07, 0x0E, 0x71, 0x0E, 0x00, 0xE1, 0xC1, 0xC0, 0x39, 0xC0, 0x00, 0x0E, 0x07,
    0xFE, 0x7E, 0x0E, 0x00, 0xFF, 0xFF, 0xC0, 0x3B, 0xC0, 0x00, 0x0E, 0x07, 0xFE, 0x3F, 0x0E, 0x00,
    0xFF, 0xFF, 0xC0, 0x1F, 0x80, 0x00, 0x0E, 0x07, 0x00, 0x1F, 0x8E, 0x00, 0xE1, 0xC1, 0xC0, 0x1F,
    0x80, 0x00, 0x0E, 0x07, 0x84, 0x43, 0x8E, 0x00, 0xE1, 0xC1, 0xC0, 0x1F, 0xC0, 0x00, 0x0E, 0x03,
    0xFC, 0x7F, 0x8F, 0x80, 0x01, 0xC0, 0x00, 0x3F, 0xE0, 0x00, 0x0E, 0x03, 0xFC, 0x7F, 0x0F, 0x80,
    0x01, 0xC0, 0x00, 0xF9, 0xF0, 0x00, 0x0E, 0x00, 0xF8, 0x7E, 0x07, 0x80, 0x01, 0xC0, 0x03, 0xE0,
    0x7C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0xC0, 0x07, 0x80, 0x3E, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x01, 0xC0, 0x02, 0x00, 0x0C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
const DATE_TEXT_WIDTH: usize = 112;
const DATE_TEXT_HEIGHT: usize = 15;
const DATE_TEXT_BYTES_PER_ROW: usize = 14;
const DATE_TEXT_BITMAP: [u8; 210] = [
    0x1E, 0x01, 0xE0, 0x1E, 0x00, 0xF8, 0x00, 0x0F, 0x00, 0x7C, 0x00, 0x07, 0x83, 0xFF, 0x7F, 0x03,
    0xF8, 0x7F, 0x03, 0xF8, 0x00, 0x1F, 0xC1, 0xFC, 0x00, 0x0F, 0xE3, 0xFF, 0x7F, 0x87, 0xF8, 0x7F,
    0x83, 0xF8, 0x00, 0x3F, 0xC1, 0xFC, 0x00, 0x1F, 0xE3, 0xFF, 0x43, 0x87, 0x3C, 0x43, 0x87, 0x88,
    0x00, 0x39, 0xE3, 0xC4, 0x00, 0x1C, 0xF0, 0x0E, 0x03, 0x8E, 0x1C, 0x03, 0x87, 0x00, 0x00, 0x70,
    0xE3, 0x80, 0x00, 0x38, 0x70, 0x0E, 0x03, 0x8E, 0x1C, 0x03, 0x8E, 0xF0, 0x00, 0x70, 0xE7, 0x78,
    0x00, 0x38, 0x70, 0x1C, 0x07, 0x8E, 0x1C, 0x07, 0x8F, 0xF8, 0x00, 0x70, 0xE7, 0xFC, 0x00, 0x38,
    0x70, 0x1C, 0x0F, 0x0E, 0x1C, 0x0F, 0x0F, 0xFC, 0x00, 0x70, 0xE7, 0xFE, 0x00, 0x38, 0x70, 0x38,
    0x1E, 0x0E, 0x1C, 0x1E, 0x0F, 0x1C, 0x7E, 0x70, 0xE7, 0x8E, 0x3F, 0x38, 0x70, 0x38, 0x3C, 0x0E,
    0x1C, 0x3C, 0x0E, 0x1C, 0x7E, 0x70, 0xE7, 0x0E, 0x3F, 0x38, 0x70, 0x70, 0x78, 0x0E, 0x1C, 0x78,
    0x0E, 0x1C, 0x00, 0x70, 0xE7, 0x0E, 0x00, 0x38, 0x70, 0x70, 0x70, 0x0F, 0x38, 0x70, 0x0F, 0x3C,
    0x00, 0x79, 0xC7, 0x9E, 0x00, 0x3C, 0xE0, 0x70, 0xFF, 0x87, 0xF8, 0xFF, 0x87, 0xF8, 0x00, 0x3F,
    0xC3, 0xFC, 0x00, 0x1F, 0xE0, 0xE0, 0xFF, 0x87, 0xF0, 0xFF, 0x83, 0xF8, 0x00, 0x3F, 0x81, 0xFC,
    0x00, 0x1F, 0xC0, 0xE0, 0xFF, 0x81, 0xE0, 0xFF, 0x81, 0xE0, 0x00, 0x0F, 0x00, 0xF0, 0x00, 0x07,
    0x80, 0xE0,
];
const NOTICE_TEXT_WIDTH: usize = 93;
const NOTICE_TEXT_HEIGHT: usize = 15;
const NOTICE_TEXT_BYTES_PER_ROW: usize = 12;
const NOTICE_TEXT_BITMAP: [u8; 180] = [
    0x0F, 0xC1, 0xE0, 0x39, 0xFE, 0x3F, 0xC0, 0xE0, 0x01, 0xE0, 0xF0, 0x78, 0x1F, 0xF0, 0xE0, 0x71,
    0xFE, 0x3F, 0xE0, 0xE0, 0x03, 0xF0, 0x70, 0x70, 0x3F, 0xF0, 0xE0, 0x71, 0xFE, 0x3F, 0xF0, 0xE0,
    0x03, 0xF0, 0x78, 0x70, 0x78, 0x78, 0xF0, 0x71, 0xC0, 0x38, 0xF0, 0xE0, 0x03, 0xF0, 0x38, 0xE0,
    0x70, 0x3C, 0x70, 0xE1, 0xC0, 0x38, 0x70, 0xE0, 0x07, 0xF8, 0x3C, 0xE0, 0xE0, 0x1C, 0x70, 0xE1,
    0xC0, 0x38, 0x70, 0xE0, 0x07, 0x38, 0x1D, 0xC0, 0xE0, 0x1C, 0x70, 0xE1, 0xFC, 0x38, 0xF0, 0xE0,
    0x07, 0x38, 0x1D, 0xC0, 0xE0, 0x1C, 0x39, 0xC1, 0xFC, 0x3F, 0xE0, 0xE0, 0x0F, 0x3C, 0x0F, 0x80,
    0xE0, 0x1C, 0x39, 0xC1, 0xFC, 0x3F, 0xC0, 0xE0, 0x0E, 0x1C, 0x0F, 0x80, 0xE0, 0x1C, 0x39, 0xC1,
    0xC0, 0x3F, 0xC0, 0xE0, 0x0F, 0xFC, 0x07, 0x00, 0xF0, 0x38, 0x1F, 0x81, 0xC0, 0x39, 0xE0, 0xE0,
    0x1F, 0xFE, 0x07, 0x00, 0x78, 0x78, 0x1F, 0x81, 0xC0, 0x38, 0xF0, 0xE0, 0x1F, 0xFE, 0x07, 0x00,
    0x3F, 0xF0, 0x1F, 0x81, 0xFE, 0x38, 0x70, 0xFF, 0x9C, 0x0E, 0x07, 0x00, 0x1F, 0xE0, 0x0F, 0x01,
    0xFE, 0x38, 0x78, 0xFF, 0xBC, 0x0F, 0x07, 0x00, 0x0F, 0xC0, 0x0F, 0x01, 0xFE, 0x38, 0x3C, 0xFF,
    0xB8, 0x0F, 0x07, 0x00,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EpdProbe {
    Refreshed,
    PhotoRefreshed,
    ImageFormatError,
    ImageReadError,
    InitError,
    BusyTimeout,
    TransportError,
}

impl EpdProbe {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Refreshed => "refreshed",
            Self::PhotoRefreshed => "photo-refreshed",
            Self::ImageFormatError => "image-format-error",
            Self::ImageReadError => "image-read-error",
            Self::InitError => "init-error",
            Self::BusyTimeout => "busy-timeout",
            Self::TransportError => "transport-error",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HardwareSelfTestReport {
    pub base: SelfTestReport,
    pub epd: EpdProbe,
    pub wifi: WifiProbe,
    pub http: HttpProbe,
    pub wake_marker: WakeMarkerProbe,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WakeMarkerProbe {
    Timer,
    Unknown,
    Missing,
    ReadError,
    WriteError,
}

impl WakeMarkerProbe {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Timer => "timer",
            Self::Unknown => "unknown",
            Self::Missing => "missing",
            Self::ReadError => "read-error",
            Self::WriteError => "write-error",
        }
    }
}

pub fn run_espidf_hardware_self_test(wake: WakeProbe) -> HardwareSelfTestReport {
    let peripherals = match esp_idf_svc::hal::peripherals::Peripherals::take() {
        Ok(peripherals) => peripherals,
        Err(_) => {
            return HardwareSelfTestReport {
                base: SelfTestReport {
                    storage: StorageProbe::MountError,
                    config: ConfigProbe::ReadError,
                    render: RenderProbe {
                        refresh_count: 0,
                        slept: false,
                    },
                },
                epd: EpdProbe::InitError,
                wifi: WifiProbe::InitError,
                http: HttpProbe::Skipped,
                wake_marker: WakeMarkerProbe::ReadError,
            };
        }
    };

    let pins = peripherals.pins;
    match init_axp2101_for_photo_painter(peripherals.i2c0, pins.gpio47, pins.gpio48) {
        Ok(probe) => {
            log::info!(
                target: "epaper_album",
                "pmic: chip=0x{:02x} axp2101={} dc=0x{:02x} ldo=0x{:02x}",
                probe.chip_id,
                chip_id_is_axp2101(probe),
                probe.dc_onoff,
                probe.ldo_onoff0
            );
        }
        Err(_) => {
            log::warn!(target: "epaper_album", "pmic: init-error");
        }
    }

    let (config_read, image_read, wake_marker) = match with_mounted_sdcard_parts(
        peripherals.sdmmc1,
        pins.gpio41,
        pins.gpio39,
        pins.gpio40,
        pins.gpio1,
        pins.gpio2,
        pins.gpio38,
        || {
            let config_read = read_text_from_mounted_path(Path::new(CONFIG_PATH));
            let image_read = read_epd_frame_from_mounted_bmp(Path::new(TEST_BMP_PATH));
            let wake_marker = probe_wake_marker(Path::new(WAKE_TEST_MARKER_PATH), wake);
            Ok((config_read, image_read, wake_marker))
        },
    ) {
        Ok(Ok(files)) => files,
        Ok(Err(_)) => (
            StorageRead::ReadError,
            StorageBinaryRead::ReadError,
            WakeMarkerProbe::ReadError,
        ),
        Err(_) => (
            StorageRead::MountError,
            StorageBinaryRead::MountError,
            WakeMarkerProbe::ReadError,
        ),
    };

    let storage = probe_storage(&config_read);
    let config = probe_config(config_read);
    let network = probe_test_network(peripherals.modem, config.value.as_ref());
    let epd = match EspEpdBus::new(
        peripherals.spi3,
        pins.gpio10,
        pins.gpio11,
        pins.gpio9,
        pins.gpio8,
        pins.gpio12,
        pins.gpio13,
    ) {
        Ok(mut bus) => refresh_epd_from_self_test_image(&mut bus, image_read),
        Err(_) => EpdProbe::InitError,
    };

    HardwareSelfTestReport {
        base: SelfTestReport {
            storage,
            config: config.probe,
            render: RenderProbe {
                refresh_count: 0,
                slept: false,
            },
        },
        epd,
        wifi: network.wifi,
        http: network.http,
        wake_marker,
    }
}

fn refresh_epd_from_self_test_image(
    bus: &mut EspEpdBus,
    image_read: StorageBinaryRead,
) -> EpdProbe {
    match image_read {
        StorageBinaryRead::Bytes(frame) => {
            log::info!(
                target: "epaper_album",
                "test.bmp: {} frame bytes, photo-refresh",
                frame.len()
            );

            match run_epd_memory_frame(bus, &frame) {
                Ok(()) => EpdProbe::PhotoRefreshed,
                Err(error) => epd_error_probe(error),
            }
        }
        StorageBinaryRead::Missing => match run_epd_hardware_self_test(bus) {
            Ok(()) => EpdProbe::Refreshed,
            Err(error) => epd_error_probe(error),
        },
        StorageBinaryRead::FormatError => EpdProbe::ImageFormatError,
        StorageBinaryRead::ReadError => EpdProbe::ImageReadError,
        StorageBinaryRead::MountError => EpdProbe::ImageReadError,
    }
}

fn read_epd_frame_from_mounted_bmp(image_path: &Path) -> StorageBinaryRead {
    match read_epd_frame_from_mounted_bmp_result(image_path) {
        Ok(frame) => StorageBinaryRead::Bytes(frame),
        Err(ReadImageError::Missing) => StorageBinaryRead::Missing,
        Err(ReadImageError::Format) => StorageBinaryRead::FormatError,
        Err(ReadImageError::Read) => StorageBinaryRead::ReadError,
    }
}

fn read_epd_frame_from_mounted_bmp_result(image_path: &Path) -> Result<Vec<u8>, ReadImageError> {
    let mut file = match std::fs::File::open(image_path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(ReadImageError::Missing);
        }
        Err(_) => return Err(ReadImageError::Read),
    };
    let total_len = file.metadata().map_err(|_| ReadImageError::Read)?.len();
    let mut header_bytes = [0u8; 54];
    file.read_exact(&mut header_bytes)
        .map_err(|_| ReadImageError::Read)?;
    let header = crate::bmp::parse_bmp_header(&header_bytes, total_len)
        .map_err(|_| ReadImageError::Format)?;
    let row_stride = crate::bmp::bmp_24bit_row_stride();
    let mut row_bgr = vec![0u8; row_stride];
    let mut frame = vec![0u8; EPD_FRAME_BYTES];

    for panel_y in 0..crate::epd::EPD_HEIGHT {
        let source_y = crate::epd::EPD_HEIGHT - 1 - panel_y;
        let bmp_file_y = if header.top_down {
            source_y
        } else {
            crate::epd::EPD_HEIGHT - 1 - source_y
        };
        let offset = header.pixel_offset + (bmp_file_y * row_stride) as u64;
        file.seek(SeekFrom::Start(offset))
            .map_err(|_| ReadImageError::Read)?;
        file.read_exact(&mut row_bgr)
            .map_err(|_| ReadImageError::Read)?;
        let row_start = panel_y * EPD_ROW_BYTES;
        let row = (&mut frame[row_start..row_start + EPD_ROW_BYTES])
            .try_into()
            .map_err(|_| ReadImageError::Read)?;
        crate::bmp::fill_epd_row_from_bgr_mirrored(&row_bgr, row)
            .map_err(|_| ReadImageError::Format)?;
    }

    draw_self_test_overlay(&mut frame);

    Ok(frame)
}

fn run_epd_memory_frame(bus: &mut EspEpdBus, frame: &[u8]) -> Result<(), crate::epd::EpdError> {
    if frame.len() != EPD_FRAME_BYTES {
        return Err(crate::epd::EpdError::Transport);
    }

    run_epd_prepacked_frame(bus, |panel_y, row| {
        let row_start = panel_y * EPD_ROW_BYTES;
        row.copy_from_slice(&frame[row_start..row_start + EPD_ROW_BYTES]);
        Ok(())
    })
}

fn draw_self_test_overlay(frame: &mut [u8]) {
    let style = TextStyle {
        foreground: Color::Black,
        background: Color::White,
        padding_x: 10,
        padding_y: 8,
        margin_x: 18,
        margin_y: 18,
        glyph_width: 10,
        glyph_height: 14,
        glyph_gap: 2,
    };

    draw_bitmap_text(
        frame,
        OverlaySlot::BottomLeft,
        &style,
        &BitmapText {
            width: TITLE_TEXT_WIDTH,
            height: TITLE_TEXT_HEIGHT,
            bytes_per_row: TITLE_TEXT_BYTES_PER_ROW,
            bitmap: &TITLE_TEXT_BITMAP,
        },
    );
    draw_bitmap_text(
        frame,
        OverlaySlot::BottomRight,
        &style,
        &BitmapText {
            width: DATE_TEXT_WIDTH,
            height: DATE_TEXT_HEIGHT,
            bytes_per_row: DATE_TEXT_BYTES_PER_ROW,
            bitmap: &DATE_TEXT_BITMAP,
        },
    );
    draw_bitmap_text(
        frame,
        OverlaySlot::TopLeft,
        &style,
        &BitmapText {
            width: NOTICE_TEXT_WIDTH,
            height: NOTICE_TEXT_HEIGHT,
            bytes_per_row: NOTICE_TEXT_BYTES_PER_ROW,
            bitmap: &NOTICE_TEXT_BITMAP,
        },
    );
}

struct BitmapText<'a> {
    width: usize,
    height: usize,
    bytes_per_row: usize,
    bitmap: &'a [u8],
}

fn draw_bitmap_text(frame: &mut [u8], slot: OverlaySlot, style: &TextStyle, text: &BitmapText<'_>) {
    let block_width = text.width.saturating_add(style.padding_x.saturating_mul(2));
    let block_height = text
        .height
        .saturating_add(style.padding_y.saturating_mul(2));
    let (x, y) = frame_overlay_origin(slot, block_width, block_height, style);
    fill_frame_rect(frame, x, y, block_width, block_height, style.background);

    let bitmap_x = x.saturating_add(style.padding_x);
    let bitmap_y = y.saturating_add(style.padding_y);
    draw_bitmap_text_pixels(frame, bitmap_x, bitmap_y, style.foreground, text);
}

fn frame_overlay_origin(
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
            set_overlay_pixel(frame, pixel_x, pixel_y, color);
        }
    }
}

fn draw_bitmap_text_pixels(
    frame: &mut [u8],
    x: usize,
    y: usize,
    color: Color,
    text: &BitmapText<'_>,
) {
    for pixel_y in 0..text.height {
        for pixel_x in 0..text.width {
            let byte = text.bitmap[pixel_y * text.bytes_per_row + pixel_x / 8];
            let bit = 1 << (7 - pixel_x % 8);
            if byte & bit != 0 {
                set_overlay_pixel(
                    frame,
                    x.saturating_add(pixel_x),
                    y.saturating_add(pixel_y),
                    color,
                );
            }
        }
    }
}

fn set_overlay_pixel(frame: &mut [u8], x: usize, y: usize, color: Color) -> bool {
    if x >= SCREEN_WIDTH || y >= SCREEN_HEIGHT {
        return false;
    }

    set_packed_frame_pixel(frame, SCREEN_WIDTH - 1 - x, SCREEN_HEIGHT - 1 - y, color)
}

enum ReadImageError {
    Missing,
    Format,
    Read,
}

fn read_text_from_mounted_path(path: &Path) -> StorageRead {
    match std::fs::read_to_string(path) {
        Ok(content) => StorageRead::Text(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => StorageRead::Missing,
        Err(_) => StorageRead::ReadError,
    }
}

fn probe_wake_marker(path: &Path, wake: WakeProbe) -> WakeMarkerProbe {
    if matches!(wake, WakeProbe::Timer) && std::fs::write(path, wake.label()).is_err() {
        return WakeMarkerProbe::WriteError;
    }

    match std::fs::read_to_string(path) {
        Ok(content) if content.trim() == WakeProbe::Timer.label() => WakeMarkerProbe::Timer,
        Ok(_) => WakeMarkerProbe::Unknown,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => WakeMarkerProbe::Missing,
        Err(_) => WakeMarkerProbe::ReadError,
    }
}

fn epd_error_probe(error: crate::epd::EpdError) -> EpdProbe {
    match error {
        crate::epd::EpdError::BusyTimeout => EpdProbe::BusyTimeout,
        crate::epd::EpdError::Transport => EpdProbe::TransportError,
    }
}

pub fn print_hardware_self_test_report(report: &HardwareSelfTestReport) {
    log::info!(target: "epaper_album", "epaper-album self-test");
    log::info!(target: "epaper_album", "storage: {}", report.base.storage.label());
    log::info!(target: "epaper_album", "config: {}", report.base.config.label());
    log::info!(
        target: "epaper_album",
        "epd: {}",
        report.epd.label()
    );
    log::info!(
        target: "epaper_album",
        "wifi: {}",
        report.wifi.label()
    );
    log::info!(
        target: "epaper_album",
        "http: {}",
        report.http.label()
    );
    log::info!(
        target: "epaper_album",
        "wake marker: {}",
        report.wake_marker.label()
    );
    log::info!(
        target: "epaper_album",
        "render refresh count: {}",
        report.base.render.refresh_count
    );
    log::info!(
        target: "epaper_album",
        "render sleep: {}",
        report.base.render.slept
    );
}

fn probe_storage(config_read: &StorageRead) -> StorageProbe {
    match config_read {
        StorageRead::MountError => StorageProbe::MountError,
        StorageRead::Text(_) | StorageRead::Missing | StorageRead::ReadError => {
            StorageProbe::Available
        }
    }
}

struct ProbedConfig {
    probe: ConfigProbe,
    value: Option<Config>,
}

fn probe_config(config_read: StorageRead) -> ProbedConfig {
    match config_read {
        StorageRead::Text(content) => match toml::from_str::<Config>(&content) {
            Ok(config) if config.has_required_values() => ProbedConfig {
                probe: ConfigProbe::Valid,
                value: Some(config),
            },
            Ok(_) => ProbedConfig {
                probe: ConfigProbe::Incomplete,
                value: None,
            },
            Err(_) => ProbedConfig {
                probe: ConfigProbe::ParseError,
                value: None,
            },
        },
        StorageRead::Missing => ProbedConfig {
            probe: ConfigProbe::Missing,
            value: None,
        },
        StorageRead::ReadError | StorageRead::MountError => ProbedConfig {
            probe: ConfigProbe::ReadError,
            value: None,
        },
    }
}
