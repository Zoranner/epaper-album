use crate::screen::{Color, SCREEN_WIDTH};

pub const SELF_TEST_BAR_COUNT: usize = 12;
pub const SELF_TEST_PANEL_X: usize = 52;
pub const SELF_TEST_PANEL_Y: usize = 42;
pub const SELF_TEST_PANEL_WIDTH: usize = SCREEN_WIDTH - SELF_TEST_PANEL_X * 2;
pub const SELF_TEST_PANEL_HEIGHT: usize = 396;
pub const SELF_TEST_PANEL_BORDER: usize = 4;
pub const SELF_TEST_PANEL_INSET_X: usize = 42;
pub const SELF_TEST_COLUMN_GAP: usize = 36;
pub const SELF_TEST_COLUMN_WIDTH: usize =
    (SELF_TEST_PANEL_WIDTH - SELF_TEST_PANEL_INSET_X * 2 - SELF_TEST_COLUMN_GAP) / 2;
pub const SELF_TEST_LEFT_COLUMN_X: usize = SELF_TEST_PANEL_X + SELF_TEST_PANEL_INSET_X;
pub const SELF_TEST_RIGHT_COLUMN_X: usize =
    SELF_TEST_LEFT_COLUMN_X + SELF_TEST_COLUMN_WIDTH + SELF_TEST_COLUMN_GAP;
pub const SELF_TEST_TITLE_Y: usize = 90;
pub const SELF_TEST_SUBTITLE_Y: usize = 122;
pub const SELF_TEST_BODY_Y: usize = 161;
pub const SELF_TEST_TITLE_STEP_Y: usize = 24;
pub const SELF_TEST_LINE_STEP_Y: usize = 18;
pub const SELF_TEST_SECTION_GAP_Y: usize = 6;
pub const SELF_TEST_BAR_COLORS: [Color; SELF_TEST_BAR_COUNT] = [
    Color::Green,
    Color::Blue,
    Color::Red,
    Color::Yellow,
    Color::Black,
    Color::White,
    Color::White,
    Color::Black,
    Color::Yellow,
    Color::Red,
    Color::Blue,
    Color::Green,
];

const VALUE_MAX_CHARS: usize = 26;
const SUBTITLE_FIRMWARE_MAX_CHARS: usize = 12;
const SUBTITLE_AUTHOR_MAX_CHARS: usize = 18;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelfTestPageModel {
    pub firmware: String,
    pub author: String,
    pub wake: String,
    pub wake_marker: String,
    pub pmic: String,
    pub power: String,
    pub battery: String,
    pub low_battery: String,
    pub storage: String,
    pub config: String,
    pub cloud: String,
    pub ssid: String,
    pub wifi: String,
    pub ip: String,
    pub http: String,
    pub epd: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelfTestPageSection {
    pub title: &'static str,
    pub lines: Vec<String>,
}

pub fn self_test_bar_color_for_x(x: usize) -> Color {
    let index = x
        .min(SCREEN_WIDTH.saturating_sub(1))
        .saturating_mul(SELF_TEST_BAR_COUNT)
        / SCREEN_WIDTH;
    SELF_TEST_BAR_COLORS[index.min(SELF_TEST_BAR_COLORS.len() - 1)]
}

pub fn self_test_page_columns(model: &SelfTestPageModel) -> [Vec<SelfTestPageSection>; 2] {
    [
        vec![
            SelfTestPageSection {
                title: "SYSTEM",
                lines: vec![
                    line("WAKE", &model.wake),
                    line("MARKER", &model.wake_marker),
                    line("EXIT", "KEY CLICK"),
                ],
            },
            SelfTestPageSection {
                title: "POWER",
                lines: vec![
                    line("PMIC", &model.pmic),
                    line("INPUT", &model.power),
                    line("BATTERY", &model.battery),
                    line("LOW", &model.low_battery),
                ],
            },
            SelfTestPageSection {
                title: "DISPLAY",
                lines: vec![line("EPD", &model.epd)],
            },
        ],
        vec![
            SelfTestPageSection {
                title: "STORAGE",
                lines: vec![
                    line("TF", &model.storage),
                    line("CONFIG", &model.config),
                    line("CLOUD", &model.cloud),
                ],
            },
            SelfTestPageSection {
                title: "NETWORK",
                lines: vec![
                    line("SSID", &model.ssid),
                    line("WIFI", &model.wifi),
                    line("IP", &model.ip),
                    line("HTTP", &model.http),
                ],
            },
        ],
    ]
}

pub fn self_test_page_subtitle(model: &SelfTestPageModel) -> String {
    format!(
        "FW {}  AUTHOR {}",
        fit_page_value(&model.firmware, SUBTITLE_FIRMWARE_MAX_CHARS),
        fit_page_value(&model.author, SUBTITLE_AUTHOR_MAX_CHARS)
    )
}

fn line(label: &str, value: &str) -> String {
    format!("{label}: {}", fit_page_value(value, VALUE_MAX_CHARS))
}

pub fn fit_page_value(value: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_graphic() || character == ' ' {
                character
            } else {
                '?'
            }
        })
        .collect::<String>();

    let char_count = sanitized.chars().count();
    if char_count <= max_chars {
        return sanitized;
    }

    sanitized
        .chars()
        .take(max_chars.saturating_sub(1))
        .chain(['~'])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_twelve_vertical_color_bars_across_screen_width() {
        let samples = (0..SELF_TEST_BAR_COUNT)
            .map(|index| {
                self_test_bar_color_for_x((index * 2 + 1) * SCREEN_WIDTH / SELF_TEST_BAR_COUNT / 2)
            })
            .collect::<Vec<_>>();

        assert_eq!(samples, SELF_TEST_BAR_COLORS);
        assert_eq!(self_test_bar_color_for_x(SCREEN_WIDTH), Color::Green);
    }

    #[test]
    fn page_sections_keep_only_actionable_self_test_fields() {
        let model = SelfTestPageModel {
            firmware: "0.1.0".to_string(),
            author: "Zoranner".to_string(),
            wake: "external".to_string(),
            wake_marker: "timer".to_string(),
            pmic: "0X4A AXP2101".to_string(),
            power: "VBUS=YES BAT=YES".to_string(),
            battery: "83% CHARGING".to_string(),
            low_battery: "RAW=NO EFFECTIVE=NO".to_string(),
            storage: "available".to_string(),
            config: "valid".to_string(),
            cloud: "https://device.example/api".to_string(),
            ssid: "Office-WiFi".to_string(),
            wifi: "connected".to_string(),
            ip: "192.168.1.50".to_string(),
            http: "fetched".to_string(),
            epd: "refreshed".to_string(),
        };

        let columns = self_test_page_columns(&model);
        let titles = columns
            .iter()
            .flat_map(|column| column.iter().map(|section| section.title))
            .collect::<Vec<_>>();
        let lines = columns
            .iter()
            .flat_map(|column| column.iter().flat_map(|section| section.lines.iter()))
            .collect::<Vec<_>>();

        assert_eq!(titles, ["SYSTEM", "POWER", "DISPLAY", "STORAGE", "NETWORK"]);
        assert!(lines.iter().any(|line| *line == "SSID: Office-WiFi"));
        assert!(lines.iter().any(|line| *line == "BATTERY: 83% CHARGING"));
        assert!(lines.iter().any(|line| *line == "EXIT: KEY CLICK"));
        assert!(!lines.iter().any(|line| line.contains("secret")));
    }

    #[test]
    fn page_subtitle_carries_firmware_identity_under_title() {
        let model = SelfTestPageModel {
            firmware: "0.1.0".to_string(),
            author: "Zoranner".to_string(),
            wake: "external".to_string(),
            wake_marker: "timer".to_string(),
            pmic: "0X4A AXP2101".to_string(),
            power: "VBUS=YES BAT=YES".to_string(),
            battery: "83% CHARGING".to_string(),
            low_battery: "RAW=NO EFFECTIVE=NO".to_string(),
            storage: "available".to_string(),
            config: "valid".to_string(),
            cloud: "https://device.example/api".to_string(),
            ssid: "Office-WiFi".to_string(),
            wifi: "connected".to_string(),
            ip: "192.168.1.50".to_string(),
            http: "fetched".to_string(),
            epd: "refreshed".to_string(),
        };

        assert_eq!(self_test_page_subtitle(&model), "FW 0.1.0  AUTHOR Zoranner");
    }

    #[test]
    fn page_layout_keeps_columns_centered_inside_panel() {
        const CONTENT_HEIGHT: usize = 299;
        let top_margin = SELF_TEST_TITLE_Y - SELF_TEST_PANEL_Y;
        let bottom_margin =
            SELF_TEST_PANEL_Y + SELF_TEST_PANEL_HEIGHT - (SELF_TEST_TITLE_Y + CONTENT_HEIGHT);

        assert_eq!(SELF_TEST_PANEL_WIDTH, 696);
        assert_eq!(SELF_TEST_PANEL_HEIGHT, 396);
        assert_eq!(SELF_TEST_COLUMN_WIDTH, 288);
        assert_eq!(SELF_TEST_LEFT_COLUMN_X, 94);
        assert_eq!(SELF_TEST_RIGHT_COLUMN_X, 418);
        assert_eq!(
            SELF_TEST_RIGHT_COLUMN_X + SELF_TEST_COLUMN_WIDTH,
            SELF_TEST_PANEL_X + SELF_TEST_PANEL_WIDTH - SELF_TEST_PANEL_INSET_X
        );
        assert!(top_margin.abs_diff(bottom_margin) <= 1);
    }

    #[test]
    fn page_values_are_ascii_safe_and_bounded() {
        assert_eq!(fit_page_value("家庭网络", 6), "????");
        assert_eq!(fit_page_value("abcdefghijklmnopqrstuvwxyz", 8), "abcdefg~");
    }
}
