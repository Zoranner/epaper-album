use crate::config::Config;
use crate::display::{DisplayRefreshMode, EpaperDisplay, MockDisplay};
use crate::render::{render_photo_page, RenderInput, RenderStatusHint};
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigProbe {
    Valid,
    Incomplete,
    Missing,
    ParseError,
    ReadError,
}

impl ConfigProbe {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::Incomplete => "incomplete",
            Self::Missing => "missing",
            Self::ParseError => "parse-error",
            Self::ReadError => "read-error",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenderProbe {
    pub refresh_count: u32,
    pub slept: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelfTestReport {
    pub config: ConfigProbe,
    pub render: RenderProbe,
}

pub fn run_self_test(config_path: impl AsRef<Path>) -> SelfTestReport {
    let config = probe_config(config_path.as_ref());
    let render = probe_render();

    SelfTestReport { config, render }
}

pub fn print_self_test_report(report: &SelfTestReport) {
    println!("epaper-album self-test");
    println!("config: {}", report.config.label());
    println!("render refresh count: {}", report.render.refresh_count);
    println!("render sleep: {}", report.render.slept);
}

fn probe_config(config_path: &Path) -> ConfigProbe {
    match std::fs::read_to_string(config_path) {
        Ok(content) => match toml::from_str::<Config>(&content) {
            Ok(config) if config.has_required_values() => ConfigProbe::Valid,
            Ok(_) => ConfigProbe::Incomplete,
            Err(_) => ConfigProbe::ParseError,
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => ConfigProbe::Missing,
        Err(_) => ConfigProbe::ReadError,
    }
}

fn probe_render() -> RenderProbe {
    let frame = render_photo_page(
        &RenderInput::new("SELF TEST", "2026-06-07").with_status_hint(RenderStatusHint::Offline),
    );
    let mut display = MockDisplay::new();

    if display.init().is_ok() {
        let _ = display.refresh(&frame, DisplayRefreshMode::Full);
        let _ = display.sleep();
    }

    RenderProbe {
        refresh_count: display.refresh_count(),
        slept: display.is_sleeping(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_missing_config_and_keeps_render_probe_ready() {
        let report = run_self_test("missing-config.toml");

        assert_eq!(report.config, ConfigProbe::Missing);
        assert_eq!(report.render.refresh_count, 1);
        assert!(report.render.slept);
    }

    #[test]
    fn reports_valid_config_when_required_values_exist() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        std::fs::write(
            &config_path,
            r#"
wifi_ssid = "Home WiFi"
wifi_password = "password"
base_url = "https://example.com/epaper"
secret-key = "local-secret-key"
"#,
        )
        .unwrap();

        let report = run_self_test(&config_path);

        assert_eq!(report.config, ConfigProbe::Valid);
    }

    #[test]
    fn reports_incomplete_config_when_required_value_is_blank() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        std::fs::write(
            &config_path,
            r#"
wifi_ssid = "Home WiFi"
wifi_password = "password"
base_url = ""
secret-key = "local-secret-key"
"#,
        )
        .unwrap();

        let report = run_self_test(&config_path);

        assert_eq!(report.config, ConfigProbe::Incomplete);
    }
}
