#[cfg(target_os = "espidf")]
use crate::app::RunTrigger;

#[cfg(target_os = "espidf")]
use crate::cloud::espidf::EspIdfHttpClient;
#[cfg(target_os = "espidf")]
use crate::config::{Config, CONFIG_PATH};
#[cfg(target_os = "espidf")]
use crate::device_display::{MountedSdCardDisplayResourceReader, PackedFrameDisplay};
#[cfg(target_os = "espidf")]
use crate::device_runtime::{
    run_device_cycle, DeviceCloudSync, DeviceCycleInput, DeviceCycleOutcome, DeviceCycleResult,
    DeviceDisplay, ErrorRefreshRequest, SyncRequest, SyncResult,
};
#[cfg(target_os = "espidf")]
use crate::device_sync::{CloudResourceSync, DeviceSyncError};
#[cfg(target_os = "espidf")]
use crate::epd::espidf::EspEpdBus;
#[cfg(target_os = "espidf")]
use crate::model::LocalDate;
#[cfg(target_os = "espidf")]
use crate::pmic::espidf::{init_axp2101_for_photo_painter, status_summary};
#[cfg(target_os = "espidf")]
use crate::power::{next_wakeup_sleep_plan, BatteryStatus, PowerProfile, SleepPlan};
#[cfg(target_os = "espidf")]
use crate::schedule::next_plan_change_date;
#[cfg(target_os = "espidf")]
use crate::state::PersistentDeviceState;
#[cfg(target_os = "espidf")]
use crate::storage::{
    read_json_file_mounted, read_text_file_mounted, with_mounted_sdcard_parts,
    write_json_file_atomic_mounted, MountedSdCardResourceStore, StorageJsonRead, StorageJsonWrite,
    StorageRead, PLAN_PATH, STATE_PATH,
};
#[cfg(target_os = "espidf")]
use crate::wifi::espidf::{connect_wifi, ConnectedWifi, WifiConnectError};
#[cfg(target_os = "espidf")]
use core::time::Duration;

#[cfg(target_os = "espidf")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspDeviceRunReport {
    pub outcome: EspDeviceRunOutcome,
    pub cycle: Option<DeviceCycleResult>,
    pub sleep_plan: Option<SleepPlan>,
}

#[cfg(target_os = "espidf")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EspDeviceRunOutcome {
    Completed(DeviceCycleOutcome),
    PeripheralInitError,
    StorageMountError,
    EpdInitError,
    StateWriteError,
}

#[cfg(target_os = "espidf")]
impl EspDeviceRunOutcome {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Completed(_) => "completed",
            Self::PeripheralInitError => "peripheral-init-error",
            Self::StorageMountError => "storage-mount-error",
            Self::EpdInitError => "epd-init-error",
            Self::StateWriteError => "state-write-error",
        }
    }
}

#[cfg(target_os = "espidf")]
pub fn run_espidf_device_cycle(trigger: RunTrigger) -> EspDeviceRunReport {
    let peripherals = match esp_idf_svc::hal::peripherals::Peripherals::take() {
        Ok(peripherals) => peripherals,
        Err(_) => {
            return EspDeviceRunReport {
                outcome: EspDeviceRunOutcome::PeripheralInitError,
                cycle: None,
                sleep_plan: None,
            };
        }
    };

    let pins = peripherals.pins;
    let pmic_probe = match init_axp2101_for_photo_painter(
        peripherals.i2c0,
        pins.gpio47,
        pins.gpio48,
    ) {
        Ok(probe) => {
            let pmic_status = status_summary(probe.status1, probe.status2);
            log::info!(
                target: "epaper_album",
                "pmic: chip=0x{:02x} status1=0x{:02x} status2=0x{:02x} vbus={} battery-present={} current-dir={} charge-step={} battery={:?} percent={:?} low={}",
                probe.chip_id,
                probe.status1,
                probe.status2,
                pmic_status.vbus_good,
                pmic_status.battery_connected,
                pmic_status.battery_current_direction,
                pmic_status.charge_step,
                probe.battery.charge_state,
                probe.battery.percent,
                probe.battery.low_battery
            );
            Some(probe)
        }
        Err(error) => {
            log::warn!(target: "epaper_album", "pmic: init-error: {error:?}");
            None
        }
    };

    let mut now_epoch_seconds = current_epoch_seconds();
    let mut date = today();
    let epd_bus = match EspEpdBus::new(
        peripherals.spi3,
        pins.gpio10,
        pins.gpio11,
        pins.gpio9,
        pins.gpio8,
        pins.gpio12,
        pins.gpio13,
    ) {
        Ok(epd_bus) => epd_bus,
        Err(_) => {
            return EspDeviceRunReport {
                outcome: EspDeviceRunOutcome::EpdInitError,
                cycle: None,
                sleep_plan: None,
            };
        }
    };
    let mut display = PackedFrameDisplay::new(MountedSdCardDisplayResourceReader, epd_bus);

    let result = with_mounted_sdcard_parts(
        peripherals.sdmmc1,
        pins.gpio41,
        pins.gpio39,
        pins.gpio40,
        pins.gpio1,
        pins.gpio2,
        pins.gpio38,
        || {
            let config = read_config_mounted();
            let plans = read_optional_json_mounted(PLAN_PATH);
            let persistent_state = read_optional_json_mounted(STATE_PATH)
                .unwrap_or_else(PersistentDeviceState::default);
            let battery = pmic_probe
                .map(|probe| probe.battery)
                .unwrap_or_else(BatteryStatus::unknown);
            let mut sync = EspDeviceCloudSync::new(peripherals.modem);

            if let Some(config) = config
                .as_ref()
                .filter(|config| config.has_required_values() && !battery.effective_low_battery())
            {
                sync.prepare_network(config);
                now_epoch_seconds = current_epoch_seconds();
                date = today();
                log::info!(
                    target: "epaper_album",
                    "time: unix={} date={}",
                    now_epoch_seconds,
                    date
                );
            }

            let power_profile = PowerProfile::from(&battery);
            log::info!(
                target: "epaper_album",
                "power: profile={:?} wake-interval={:?} battery={:?} percent={:?} low={}",
                power_profile,
                power_profile.wake_interval_seconds(),
                battery.charge_state,
                battery.percent,
                battery.low_battery
            );
            let cycle = run_device_cycle(
                DeviceCycleInput {
                    config,
                    plans,
                    persistent_state,
                    trigger,
                    now_epoch_seconds,
                    date,
                    battery,
                },
                &mut sync,
                &mut display,
            );

            if let Err(outcome) = write_cycle_files(&cycle) {
                return Ok(Err(outcome));
            }
            let sleep_plan = build_sleep_plan(&cycle, now_epoch_seconds, date);
            Ok(Ok((cycle, sleep_plan)))
        },
    );

    match result {
        Ok(Ok(Ok((cycle, sleep_plan)))) => EspDeviceRunReport {
            outcome: EspDeviceRunOutcome::Completed(cycle.outcome.clone()),
            cycle: Some(cycle),
            sleep_plan: Some(sleep_plan),
        },
        Ok(Ok(Err(outcome))) => EspDeviceRunReport {
            outcome,
            cycle: None,
            sleep_plan: None,
        },
        Ok(Err(_)) | Err(_) => {
            refresh_storage_error_page(&mut display, now_epoch_seconds);
            EspDeviceRunReport {
                outcome: EspDeviceRunOutcome::StorageMountError,
                cycle: None,
                sleep_plan: None,
            }
        }
    }
}

#[cfg(target_os = "espidf")]
fn refresh_storage_error_page<D>(display: &mut D, now_epoch_seconds: u64)
where
    D: DeviceDisplay,
{
    let result = display.refresh_error_page(ErrorRefreshRequest {
        title: "STORAGE ERROR".to_string(),
        message: "TF CARD IS NOT AVAILABLE".to_string(),
        hint: "CHECK TF CARD AND SLOT".to_string(),
        detail: "MOUNT /SDCARD FAILED".to_string(),
        now_epoch_seconds,
    });

    if let Err(error) = result {
        log::warn!(target: "epaper_album", "storage error page: {error}");
    }
}

#[cfg(target_os = "espidf")]
struct EspDeviceCloudSync {
    modem: Option<esp_idf_svc::hal::modem::Modem<'static>>,
    wifi: Option<ConnectedWifi>,
    sntp: Option<esp_idf_svc::sntp::EspSntp<'static>>,
    time_synced: bool,
    inner: CloudResourceSync<EspIdfHttpClient, MountedSdCardResourceStore>,
}

#[cfg(target_os = "espidf")]
impl EspDeviceCloudSync {
    fn new(modem: esp_idf_svc::hal::modem::Modem<'static>) -> Self {
        Self {
            modem: Some(modem),
            wifi: None,
            sntp: None,
            time_synced: false,
            inner: CloudResourceSync::new(EspIdfHttpClient, MountedSdCardResourceStore),
        }
    }

    fn prepare_network(&mut self, config: &Config) {
        if self.wifi.is_none() {
            let Some(modem) = self.modem.take() else {
                log::warn!(target: "epaper_album", "wifi: modem-unavailable");
                return;
            };

            match connect_wifi(modem, config) {
                Ok(wifi) => {
                    self.wifi = Some(wifi);
                }
                Err(error) => {
                    log::warn!(target: "epaper_album", "wifi: {error:?}");
                    return;
                }
            }
        }

        self.sync_time();
    }

    fn sync_time(&mut self) {
        if self.time_synced {
            return;
        }

        if self.sntp.is_none() {
            match esp_idf_svc::sntp::EspSntp::new_default() {
                Ok(sntp) => {
                    self.sntp = Some(sntp);
                    log::info!(target: "epaper_album", "sntp: started");
                }
                Err(error) => {
                    log::warn!(target: "epaper_album", "sntp: init-error: {error:?}");
                    return;
                }
            }
        }

        let Some(sntp) = self.sntp.as_ref() else {
            return;
        };

        for _ in 0..20 {
            if sntp.get_sync_status() == esp_idf_svc::sntp::SyncStatus::Completed {
                log::info!(target: "epaper_album", "sntp: completed");
                self.time_synced = true;
                return;
            }
            std::thread::sleep(Duration::from_millis(500));
        }

        log::warn!(
            target: "epaper_album",
            "sntp: timeout status={:?}",
            sntp.get_sync_status()
        );
    }
}

#[cfg(target_os = "espidf")]
#[derive(Debug)]
enum EspDeviceSyncError {
    Wifi(WifiConnectError),
    Sync(DeviceSyncError),
}

#[cfg(target_os = "espidf")]
impl core::fmt::Display for EspDeviceSyncError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Wifi(error) => write!(formatter, "wifi: {error:?}"),
            Self::Sync(error) => write!(formatter, "{error}"),
        }
    }
}

#[cfg(target_os = "espidf")]
impl DeviceCloudSync for EspDeviceCloudSync {
    type Error = EspDeviceSyncError;

    fn sync_resources(&mut self, request: SyncRequest) -> Result<SyncResult, Self::Error> {
        self.prepare_network(&request.config);
        if self.wifi.is_none() {
            return Err(EspDeviceSyncError::Wifi(WifiConnectError::InitError));
        }

        self.inner
            .sync_resources(request)
            .map_err(EspDeviceSyncError::Sync)
    }
}

#[cfg(target_os = "espidf")]
fn read_config_mounted() -> Option<Config> {
    match read_text_file_mounted(CONFIG_PATH) {
        StorageRead::Text(content) => match toml::from_str::<Config>(&content) {
            Ok(config) if config.has_required_values() => Some(config),
            Ok(_) | Err(_) => None,
        },
        StorageRead::Missing | StorageRead::MountError | StorageRead::ReadError => None,
    }
}

#[cfg(target_os = "espidf")]
fn read_optional_json_mounted<T>(path: &str) -> Option<T>
where
    T: serde::de::DeserializeOwned,
{
    match read_json_file_mounted(path) {
        StorageJsonRead::Value(value) => Some(value),
        StorageJsonRead::Missing
        | StorageJsonRead::MountError
        | StorageJsonRead::ReadError
        | StorageJsonRead::ParseError => None,
    }
}

#[cfg(target_os = "espidf")]
fn write_cycle_files(cycle: &DeviceCycleResult) -> Result<(), EspDeviceRunOutcome> {
    if let Some(plans) = &cycle.plans {
        write_json_checked(PLAN_PATH, plans)?;
    }
    write_json_checked(STATE_PATH, &cycle.persistent_state)?;
    Ok(())
}

#[cfg(target_os = "espidf")]
fn write_json_checked<T>(path: &str, value: &T) -> Result<(), EspDeviceRunOutcome>
where
    T: serde::Serialize,
{
    match write_json_file_atomic_mounted(path, value) {
        StorageJsonWrite::Written => Ok(()),
        StorageJsonWrite::SerializeError
        | StorageJsonWrite::MountError
        | StorageJsonWrite::WriteError => Err(EspDeviceRunOutcome::StateWriteError),
    }
}

#[cfg(target_os = "espidf")]
fn build_sleep_plan(
    cycle: &DeviceCycleResult,
    now_epoch_seconds: u64,
    date: LocalDate,
) -> SleepPlan {
    let power_profile = PowerProfile::from(&cycle.battery);
    let next_wake = now_epoch_seconds.saturating_add(power_profile.wake_interval_seconds());
    let next_plan_change = cycle
        .plans
        .as_ref()
        .and_then(|plans| next_plan_change_date(plans, date))
        .map(local_date_start_epoch_seconds);

    next_wakeup_sleep_plan(now_epoch_seconds, next_wake, next_plan_change, None)
}

#[cfg(target_os = "espidf")]
fn current_epoch_seconds() -> u64 {
    chrono::Utc::now().timestamp().max(0) as u64
}

#[cfg(target_os = "espidf")]
fn today() -> LocalDate {
    use chrono::{Datelike, TimeZone, Utc};

    let timestamp = chrono::Utc::now()
        .timestamp()
        .saturating_add(8 * 60 * 60)
        .max(0);
    let now = Utc
        .timestamp_opt(timestamp, 0)
        .single()
        .unwrap_or_else(Utc::now);
    LocalDate::new(now.year() as u16, now.month() as u8, now.day() as u8)
        .unwrap_or_else(|| LocalDate::parse("2026-01-01").unwrap())
}

#[cfg(target_os = "espidf")]
fn local_date_start_epoch_seconds(date: LocalDate) -> u64 {
    use chrono::{Local, TimeZone};

    Local
        .with_ymd_and_hms(
            date.year as i32,
            u32::from(date.month),
            u32::from(date.day),
            0,
            0,
            0,
        )
        .single()
        .map(|datetime| datetime.timestamp().max(0) as u64)
        .unwrap_or(0)
}
