use crate::config::{Config, CONFIG_PATH};
use crate::epd::{espidf::EspEpdBus, run_epd_hardware_self_test};
use crate::selftest::{ConfigProbe, RenderProbe, SelfTestReport, StorageProbe};
use crate::storage::{read_espidf_text_file_with_sdmmc, StorageRead};
use std::path::Path;

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
}

pub fn run_espidf_hardware_self_test() -> HardwareSelfTestReport {
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
            };
        }
    };

    let pins = peripherals.pins;
    let config_read = read_espidf_text_file_with_sdmmc(
        Path::new(CONFIG_PATH),
        peripherals.sdmmc1,
        pins.gpio41,
        pins.gpio39,
        pins.gpio40,
        pins.gpio1,
        pins.gpio2,
        pins.gpio38,
    );
    let storage = probe_storage(&config_read);
    let config = probe_config(config_read);
    let epd = match EspEpdBus::new(
        peripherals.spi3,
        pins.gpio10,
        pins.gpio11,
        pins.gpio9,
        pins.gpio8,
        pins.gpio12,
        pins.gpio13,
    ) {
        Ok(mut bus) => match run_epd_hardware_self_test(&mut bus) {
            Ok(()) => EpdProbe::Refreshed,
            Err(crate::epd::EpdError::BusyTimeout) => EpdProbe::BusyTimeout,
            Err(crate::epd::EpdError::Transport) => EpdProbe::TransportError,
        },
        Err(_) => EpdProbe::InitError,
    };

    HardwareSelfTestReport {
        base: SelfTestReport {
            storage,
            config,
            render: RenderProbe {
                refresh_count: 0,
                slept: false,
            },
        },
        epd,
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
