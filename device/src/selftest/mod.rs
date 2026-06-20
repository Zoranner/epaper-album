#[cfg(target_os = "espidf")]
pub mod display;
#[cfg(target_os = "espidf")]
pub mod hardware;
pub mod page;
#[cfg(target_os = "espidf")]
pub mod report;
#[cfg(target_os = "espidf")]
pub mod wake_marker;

use crate::config::Config;
#[cfg(not(target_os = "espidf"))]
use crate::render::{render_photo_page, RenderInput};
#[cfg(not(target_os = "espidf"))]
use crate::screen::{DisplayRefreshMode, EpaperDisplay, MockDisplay};
use crate::storage::{read_text_file, StorageRead};
use std::fmt::Arguments;
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
pub enum StorageProbe {
    Available,
    MountError,
}

impl StorageProbe {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::MountError => "mount-error",
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
    pub storage: StorageProbe,
    pub config: ConfigProbe,
    pub render: RenderProbe,
}

pub fn run_self_test(config_path: impl AsRef<Path>) -> SelfTestReport {
    let config_read = read_text_file(config_path);
    let storage = probe_storage(&config_read);
    let config = probe_config(config_read);
    let render = probe_render();

    SelfTestReport {
        storage,
        config,
        render,
    }
}

pub fn print_self_test_report(report: &SelfTestReport) {
    print_report_line(format_args!("Inkframe self-test"));
    print_report_line(format_args!("storage: {}", report.storage.label()));
    print_report_line(format_args!("config: {}", report.config.label()));
    print_report_line(format_args!(
        "render refresh count: {}",
        report.render.refresh_count
    ));
    print_report_line(format_args!("render sleep: {}", report.render.slept));
}

#[cfg(target_os = "espidf")]
fn print_report_line(args: Arguments<'_>) {
    log::info!(target: "inkframe_device", "{}", args);
}

#[cfg(not(target_os = "espidf"))]
fn print_report_line(args: Arguments<'_>) {
    println!("{}", args);
}

fn probe_storage(config_read: &StorageRead) -> StorageProbe {
    match config_read {
        StorageRead::MountError => StorageProbe::MountError,
        StorageRead::Text(_) | StorageRead::Missing | StorageRead::ReadError => {
            StorageProbe::Available
        }
    }
}

fn probe_config(config_read: StorageRead) -> ConfigProbe {
    match config_read {
        StorageRead::Text(content) => match toml::from_str::<Config>(&content) {
            Ok(config) if config.has_required_values() => ConfigProbe::Valid,
            Ok(_) => ConfigProbe::Incomplete,
            Err(_) => ConfigProbe::ParseError,
        },
        StorageRead::Missing => ConfigProbe::Missing,
        StorageRead::ReadError => ConfigProbe::ReadError,
        StorageRead::MountError => ConfigProbe::ReadError,
    }
}

fn probe_render() -> RenderProbe {
    #[cfg(target_os = "espidf")]
    {
        return RenderProbe {
            refresh_count: 0,
            slept: false,
        };
    }

    #[cfg(not(target_os = "espidf"))]
    {
        let frame = render_photo_page(&RenderInput::new("SELF TEST", "2026-06-07"));
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_missing_config_and_keeps_render_probe_ready() {
        let report = run_self_test("missing-config.toml");

        assert_eq!(report.storage, StorageProbe::Available);
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

        assert_eq!(report.storage, StorageProbe::Available);
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
