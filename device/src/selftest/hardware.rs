use crate::config::{Config, CONFIG_PATH};
use crate::epd::espidf::EspEpdBus;
use crate::pmic::espidf::{chip_id_is_axp2101, init_axp2101_for_photo_painter, PmicProbe};
use crate::pmic::status_summary;
use crate::power::espidf::WakeProbe;
use crate::power::ChargeState;
use crate::selftest::display::refresh_epd_from_self_test_report;
use crate::selftest::wake_marker::{probe_wake_marker, WakeMarkerProbe, WAKE_TEST_MARKER_PATH};
use crate::selftest::{ConfigProbe, RenderProbe, SelfTestReport, StorageProbe};
use crate::storage::{with_mounted_sdcard_parts, StorageRead};
use crate::wifi::espidf::{probe_test_network, HttpProbe, WifiProbe};
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
    pub irq_enable2_before: u8,
    pub irq_enable2_after: u8,
    pub irq_status1_before_clear: u8,
    pub irq_status2_before_clear: u8,
    pub irq_status3_before_clear: u8,
    pub dc_onoff: u8,
    pub ldo_onoff0: u8,
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
                "pmic: chip=0x{:02x} axp2101={} vbus={} battery-present={} dc=0x{:02x} ldo=0x{:02x} battery={:?} percent={:?} low={} effective-low={} irq-enable2=0x{:02x}->0x{:02x} irq-status-before=0x{:02x}/0x{:02x}/0x{:02x}",
                summary.chip_id,
                summary.is_axp2101,
                summary.vbus_good,
                summary.battery_connected,
                summary.dc_onoff,
                summary.ldo_onoff0,
                summary.charge_state,
                summary.percent,
                summary.low_battery,
                summary.effective_low_battery,
                summary.irq_enable2_before,
                summary.irq_enable2_after,
                summary.irq_status1_before_clear,
                summary.irq_status2_before_clear,
                summary.irq_status3_before_clear
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

pub fn play_self_test_key_tone() {
    if let Err(error) = crate::audio::espidf::play_self_test_request_tone() {
        log::warn!(
            target: "inkframe_device",
            "self-test key: tone error: {error:?}"
        );
    }
}

fn read_text_from_mounted_path(path: &Path) -> StorageRead {
    match std::fs::read_to_string(path) {
        Ok(content) => StorageRead::Text(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => StorageRead::Missing,
        Err(_) => StorageRead::ReadError,
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
        irq_enable2_before: probe.irq.enable2_before,
        irq_enable2_after: probe.irq.enable2_after,
        irq_status1_before_clear: probe.irq.status1_before_clear,
        irq_status2_before_clear: probe.irq.status2_before_clear,
        irq_status3_before_clear: probe.irq.status3_before_clear,
        dc_onoff: probe.dc_onoff,
        ldo_onoff0: probe.ldo_onoff0,
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
