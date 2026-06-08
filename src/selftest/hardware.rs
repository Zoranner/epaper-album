use crate::config::{Config, CONFIG_PATH};
use crate::display::{Color, SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::epd::{
    espidf::EspEpdBus, pack_epd_pixels, run_epd_packed_frame, set_logical_packed_frame_pixel,
    EPD_FRAME_BYTES,
};
use crate::pmic::espidf::{chip_id_is_axp2101, init_axp2101_for_photo_painter};
use crate::power::espidf::WakeProbe;
use crate::render::{glyph_pattern, TextStyle};
use crate::selftest::{ConfigProbe, RenderProbe, SelfTestReport, StorageProbe};
use crate::storage::{with_mounted_sdcard_parts, StorageRead};
use crate::wifi::espidf::{probe_test_network, HttpProbe, WifiProbe};
use std::path::Path;

const WAKE_TEST_MARKER_PATH: &str = "/sdcard/wake-test.txt";
const SELF_TEST_PANEL_X: usize = 80;
const SELF_TEST_PANEL_Y: usize = 76;
const SELF_TEST_PANEL_WIDTH: usize = SCREEN_WIDTH - SELF_TEST_PANEL_X * 2;
const SELF_TEST_PANEL_HEIGHT: usize = SCREEN_HEIGHT - SELF_TEST_PANEL_Y * 2;
const SELF_TEST_PANEL_BORDER: usize = 4;
const SELF_TEST_BARS: [Color; 6] = [
    Color::Green,
    Color::Blue,
    Color::Red,
    Color::Yellow,
    Color::Black,
    Color::White,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EpdProbe {
    Refreshed,
    InitError,
    BusyTimeout,
    TransportError,
}

impl EpdProbe {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Refreshed => "refreshed",
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
    pub wake: WakeProbe,
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
                wake,
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

    let (config_read, wake_marker) = match with_mounted_sdcard_parts(
        peripherals.sdmmc1,
        pins.gpio41,
        pins.gpio39,
        pins.gpio40,
        pins.gpio1,
        pins.gpio2,
        pins.gpio38,
        || {
            let config_read = read_text_from_mounted_path(Path::new(CONFIG_PATH));
            let wake_marker = probe_wake_marker(Path::new(WAKE_TEST_MARKER_PATH), wake);
            Ok((config_read, wake_marker))
        },
    ) {
        Ok(Ok(files)) => files,
        Ok(Err(_)) | Err(_) => (StorageRead::MountError, WakeMarkerProbe::ReadError),
    };

    let storage = probe_storage(&config_read);
    let config = probe_config(config_read);
    let network = probe_test_network(peripherals.modem, config.value.as_ref());
    let mut report = HardwareSelfTestReport {
        base: SelfTestReport {
            storage,
            config: config.probe,
            render: RenderProbe {
                refresh_count: 0,
                slept: false,
            },
        },
        epd: EpdProbe::Refreshed,
        wifi: network.wifi,
        http: network.http,
        wake_marker,
        wake,
    };

    report.epd = match EspEpdBus::new(
        peripherals.spi3,
        pins.gpio10,
        pins.gpio11,
        pins.gpio9,
        pins.gpio8,
        pins.gpio12,
        pins.gpio13,
    ) {
        Ok(mut bus) => refresh_epd_from_self_test_report(&mut bus, &report),
        Err(_) => EpdProbe::InitError,
    };

    report
}

fn refresh_epd_from_self_test_report(
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
        glyph_gap: 4,
    };
    let body_style = TextStyle {
        glyph_width: 11,
        glyph_height: 17,
        glyph_gap: 3,
        ..header_style
    };

    let content_x = SELF_TEST_PANEL_X + 42;
    let mut y = SELF_TEST_PANEL_Y + 34;
    draw_text_on_frame(frame, content_x, y, "EPAPER ALBUM SELF TEST", &header_style);

    y += 58;
    let lines = [
        format!("WAKE: {}", report.wake.label()),
        format!("STORAGE: {}", report.base.storage.label()),
        format!("CONFIG: {}", report.base.config.label()),
        format!("WIFI: {}", report.wifi.label()),
        format!("HTTP: {}", report.http.label()),
        format!("WAKE MARKER: {}", report.wake_marker.label()),
        format!("EPD: {}", report.epd.label()),
    ];

    for line in lines {
        draw_text_on_frame(frame, content_x, y, &line, &body_style);
        y += 34;
    }
}

fn draw_color_bars(frame: &mut [u8]) {
    let band_height = SCREEN_HEIGHT / SELF_TEST_BARS.len();
    for y in 0..SCREEN_HEIGHT {
        let color = SELF_TEST_BARS[(y / band_height).min(SELF_TEST_BARS.len() - 1)];
        for x in 0..SCREEN_WIDTH {
            set_logical_packed_frame_pixel(frame, x, y, color);
        }
    }
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
    log::info!(target: "epaper_album", "wake: {}", report.wake.label());
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
