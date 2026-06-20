use crate::screen::{Color, SCREEN_WIDTH};

pub const SELF_TEST_BAR_COUNT: usize = 12;
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelfTestPageModel {
    pub product: String,
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
                title: "IDENTITY",
                lines: vec![
                    line("PRODUCT", &model.product),
                    line("FIRMWARE", &model.firmware),
                    line("AUTHOR", &model.author),
                ],
            },
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
            product: "Inkframe".to_string(),
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

        assert_eq!(
            titles,
            ["IDENTITY", "SYSTEM", "POWER", "DISPLAY", "STORAGE", "NETWORK"]
        );
        assert!(lines.iter().any(|line| *line == "PRODUCT: Inkframe"));
        assert!(lines.iter().any(|line| *line == "FIRMWARE: 0.1.0"));
        assert!(lines.iter().any(|line| *line == "AUTHOR: Zoranner"));
        assert!(lines.iter().any(|line| *line == "SSID: Office-WiFi"));
        assert!(lines.iter().any(|line| *line == "BATTERY: 83% CHARGING"));
        assert!(lines.iter().any(|line| *line == "EXIT: KEY CLICK"));
        assert!(!lines.iter().any(|line| line.contains("secret")));
    }

    #[test]
    fn page_values_are_ascii_safe_and_bounded() {
        assert_eq!(fit_page_value("家庭网络", 6), "????");
        assert_eq!(fit_page_value("abcdefghijklmnopqrstuvwxyz", 8), "abcdefg~");
    }
}
