use crate::config::{Config, CONFIG_PATH};
use crate::epd::{
    espidf::EspEpdBus, pack_epd_pixels, run_epd_packed_frame, set_logical_packed_frame_pixel,
    EPD_FRAME_BYTES,
};
use crate::pmic::espidf::{
    chip_id_is_axp2101, init_axp2101_for_photo_painter, status_summary, PmicProbe,
};
use crate::power::espidf::WakeProbe;
use crate::power::ChargeState;
use crate::render::{glyph_pattern, TextStyle};
use crate::screen::{Color, SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::selftest::page::{
    self_test_bar_color_for_x, self_test_page_columns, self_test_page_subtitle, SelfTestPageModel,
    SelfTestPageSection, SELF_TEST_BODY_Y, SELF_TEST_LEFT_COLUMN_X, SELF_TEST_LINE_STEP_Y,
    SELF_TEST_PANEL_BORDER, SELF_TEST_PANEL_HEIGHT, SELF_TEST_PANEL_WIDTH, SELF_TEST_PANEL_X,
    SELF_TEST_PANEL_Y, SELF_TEST_RIGHT_COLUMN_X, SELF_TEST_SECTION_GAP_Y, SELF_TEST_SUBTITLE_Y,
    SELF_TEST_TITLE_STEP_Y, SELF_TEST_TITLE_Y,
};
use crate::selftest::{ConfigProbe, RenderProbe, SelfTestReport, StorageProbe};
use crate::storage::{with_mounted_sdcard_parts, StorageRead};
use crate::wifi::espidf::{probe_test_network, HttpProbe, WifiProbe};
use std::path::Path;

const WAKE_TEST_MARKER_PATH: &str = "/sdcard/wake-test.txt";
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HardwareSelfTestReport {
    pub base: SelfTestReport,
    pub epd: EpdProbe,
    pub wifi: WifiProbe,
    pub http: HttpProbe,
    pub wake_marker: WakeMarkerProbe,
    pub wake: WakeProbe,
    pub pmic: PmicSelfTestProbe,
    pub ssid: String,
    pub base_url: String,
    pub ip: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PmicSelfTestProbe {
    Ready(PmicSelfTestSummary),
    InitError,
}

impl PmicSelfTestProbe {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Ready(_) => "ready",
            Self::InitError => "init-error",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PmicSelfTestSummary {
    pub chip_id: u8,
    pub is_axp2101: bool,
    pub battery_connected: bool,
    pub vbus_good: bool,
    pub charge_state: ChargeState,
    pub percent: Option<u8>,
    pub low_battery: bool,
    pub effective_low_battery: bool,
    pub dc_onoff: u8,
    pub ldo_onoff0: u8,
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
                pmic: PmicSelfTestProbe::InitError,
                ssid: String::new(),
                base_url: String::new(),
                ip: String::new(),
            };
        }
    };

    let pins = peripherals.pins;
    let pmic = match init_axp2101_for_photo_painter(peripherals.i2c0, pins.gpio47, pins.gpio48) {
        Ok(probe) => {
            let summary = pmic_summary(probe);
            log::info!(
                target: "inkframe_device",
                "pmic: chip=0x{:02x} axp2101={} vbus={} battery-present={} dc=0x{:02x} ldo=0x{:02x} battery={:?} percent={:?} low={} effective-low={}",
                summary.chip_id,
                summary.is_axp2101,
                summary.vbus_good,
                summary.battery_connected,
                summary.dc_onoff,
                summary.ldo_onoff0,
                summary.charge_state,
                summary.percent,
                summary.low_battery,
                summary.effective_low_battery
            );
            PmicSelfTestProbe::Ready(summary)
        }
        Err(_) => {
            log::warn!(target: "inkframe_device", "pmic: init-error");
            PmicSelfTestProbe::InitError
        }
    };

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
    let ssid = config
        .value
        .as_ref()
        .map(|config| config.wifi_ssid.trim().to_string())
        .unwrap_or_default();
    let base_url = config
        .value
        .as_ref()
        .map(|config| config.base_url.trim().to_string())
        .unwrap_or_default();
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
        pmic,
        ssid,
        base_url,
        ip: network.ip,
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
    log::info!(target: "inkframe_device", "Inkframe self-test");
    log::info!(target: "inkframe_device", "wake: {}", report.wake.label());
    log::info!(target: "inkframe_device", "pmic: {}", report.pmic.label());
    if let PmicSelfTestProbe::Ready(summary) = &report.pmic {
        log::info!(
            target: "inkframe_device",
            "power: chip=0x{:02x} axp2101={} vbus={} battery-present={} battery={:?} percent={:?} low={} effective-low={} dc=0x{:02x} ldo=0x{:02x}",
            summary.chip_id,
            summary.is_axp2101,
            summary.vbus_good,
            summary.battery_connected,
            summary.charge_state,
            summary.percent,
            summary.low_battery,
            summary.effective_low_battery,
            summary.dc_onoff,
            summary.ldo_onoff0
        );
    }
    log::info!(target: "inkframe_device", "storage: {}", report.base.storage.label());
    log::info!(target: "inkframe_device", "config: {}", report.base.config.label());
    log::info!(target: "inkframe_device", "base url: {}", report.base_url);
    log::info!(
        target: "inkframe_device",
        "epd: {}",
        report.epd.label()
    );
    log::info!(
        target: "inkframe_device",
        "wifi: {}",
        report.wifi.label()
    );
    log::info!(target: "inkframe_device", "wifi ssid: {}", report.ssid);
    log::info!(target: "inkframe_device", "wifi ip: {}", report.ip);
    log::info!(
        target: "inkframe_device",
        "http: {}",
        report.http.label()
    );
    log::info!(
        target: "inkframe_device",
        "wake marker: {}",
        report.wake_marker.label()
    );
    log::info!(
        target: "inkframe_device",
        "render refresh count: {}",
        report.base.render.refresh_count
    );
    log::info!(
        target: "inkframe_device",
        "render sleep: {}",
        report.base.render.slept
    );
}

impl From<&HardwareSelfTestReport> for SelfTestPageModel {
    fn from(report: &HardwareSelfTestReport) -> Self {
        Self {
            firmware: env!("CARGO_PKG_VERSION").to_string(),
            author: package_authors(),
            wake: report.wake.label().to_string(),
            wake_marker: report.wake_marker.label().to_string(),
            pmic: pmic_page_value(&report.pmic),
            power: power_page_value(&report.pmic),
            battery: battery_page_value(&report.pmic),
            low_battery: low_battery_page_value(&report.pmic),
            storage: report.base.storage.label().to_string(),
            config: report.base.config.label().to_string(),
            cloud: empty_as_dash(&report.base_url),
            ssid: empty_as_dash(&report.ssid),
            wifi: report.wifi.label().to_string(),
            ip: empty_as_dash(&report.ip),
            http: report.http.label().to_string(),
            epd: report.epd.label().to_string(),
        }
    }
}

fn pmic_summary(probe: PmicProbe) -> PmicSelfTestSummary {
    let status = status_summary(probe.status1, probe.status2);
    PmicSelfTestSummary {
        chip_id: probe.chip_id,
        is_axp2101: chip_id_is_axp2101(probe),
        battery_connected: status.battery_connected,
        vbus_good: status.vbus_good,
        charge_state: probe.battery.charge_state,
        percent: probe.battery.percent,
        low_battery: probe.battery.low_battery,
        effective_low_battery: probe.battery.effective_low_battery(),
        dc_onoff: probe.dc_onoff,
        ldo_onoff0: probe.ldo_onoff0,
    }
}

fn pmic_page_value(pmic: &PmicSelfTestProbe) -> String {
    match pmic {
        PmicSelfTestProbe::Ready(summary) => {
            format!(
                "0X{:02X} {}",
                summary.chip_id,
                if summary.is_axp2101 {
                    "AXP2101"
                } else {
                    "UNKNOWN"
                }
            )
        }
        PmicSelfTestProbe::InitError => "init-error".to_string(),
    }
}

fn power_page_value(pmic: &PmicSelfTestProbe) -> String {
    match pmic {
        PmicSelfTestProbe::Ready(summary) => format!(
            "VBUS={} BAT={}",
            yes_no(summary.vbus_good),
            yes_no(summary.battery_connected)
        ),
        PmicSelfTestProbe::InitError => "-".to_string(),
    }
}

fn battery_page_value(pmic: &PmicSelfTestProbe) -> String {
    match pmic {
        PmicSelfTestProbe::Ready(summary) => format!(
            "{} {}",
            summary
                .percent
                .map(|percent| format!("{percent}%"))
                .unwrap_or_else(|| "-".to_string()),
            charge_state_label(summary.charge_state)
        ),
        PmicSelfTestProbe::InitError => "-".to_string(),
    }
}

fn low_battery_page_value(pmic: &PmicSelfTestProbe) -> String {
    match pmic {
        PmicSelfTestProbe::Ready(summary) => format!(
            "RAW={} EFFECTIVE={}",
            yes_no(summary.low_battery),
            yes_no(summary.effective_low_battery)
        ),
        PmicSelfTestProbe::InitError => "-".to_string(),
    }
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "YES"
    } else {
        "NO"
    }
}

fn empty_as_dash(value: &str) -> String {
    if value.trim().is_empty() {
        "-".to_string()
    } else {
        value.trim().to_string()
    }
}

fn charge_state_label(value: ChargeState) -> &'static str {
    match value {
        ChargeState::Unknown => "UNKNOWN",
        ChargeState::Discharging => "DISCHARGING",
        ChargeState::Charging => "CHARGING",
        ChargeState::Full => "FULL",
    }
}

fn package_authors() -> String {
    let authors = env!("CARGO_PKG_AUTHORS").replace(':', ", ");
    if authors.trim().is_empty() {
        "-".to_string()
    } else {
        authors
    }
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
