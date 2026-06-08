#[cfg(target_os = "espidf")]
use crate::app::RunTrigger;

#[cfg(target_os = "espidf")]
use crate::app_storage::DEVICE_STATE_PATH;
#[cfg(target_os = "espidf")]
use crate::cloud::espidf::EspIdfHttpClient;
#[cfg(target_os = "espidf")]
use crate::config::{Config, CONFIG_PATH};
#[cfg(target_os = "espidf")]
use crate::device_display::{MountedSdCardDisplayResourceReader, PackedFrameDisplay};
#[cfg(target_os = "espidf")]
use crate::device_runtime::{
    run_device_cycle, DeviceCloudSync, DeviceCycleInput, DeviceCycleOutcome, DeviceCycleResult,
    SyncRequest, SyncResult,
};
#[cfg(target_os = "espidf")]
use crate::device_sync::{CloudResourceSync, DeviceSyncError};
#[cfg(target_os = "espidf")]
use crate::epd::espidf::EspEpdBus;
#[cfg(target_os = "espidf")]
use crate::model::{LocalDate, ResourceIndex};
#[cfg(target_os = "espidf")]
use crate::pmic::espidf::init_axp2101_for_photo_painter;
#[cfg(target_os = "espidf")]
use crate::power::{
    next_profile_sync_epoch_seconds, next_wakeup_sleep_plan, profile_sync_due, BatteryStatus,
    PowerProfile, SleepPlan,
};
#[cfg(target_os = "espidf")]
use crate::schedule::{next_plan_change_date, DAILY_SYNC_INTERVAL_SECONDS};
#[cfg(target_os = "espidf")]
use crate::state::PersistentDeviceState;
#[cfg(target_os = "espidf")]
use crate::storage::{
    read_json_file_mounted, read_text_file_mounted, with_mounted_sdcard_parts,
    write_json_file_atomic_mounted, MountedSdCardResourceStore, StorageJsonRead, StorageJsonWrite,
    StorageRead, CACHE_INDEX_PATH, DISPLAY_STATE_PATH, PLANS_CURRENT_PATH,
};
#[cfg(target_os = "espidf")]
use crate::wifi::espidf::{connect_wifi, ConnectedWifi, WifiConnectError};

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
    let pmic_result = init_axp2101_for_photo_painter(peripherals.i2c0, pins.gpio47, pins.gpio48);
    if let Err(error) = pmic_result {
        log::warn!(target: "epaper_album", "pmic: init-error: {error:?}");
    }

    let now_epoch_seconds = now_epoch_seconds();
    let date = today();

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
            let snapshot = read_optional_json_mounted(PLANS_CURRENT_PATH);
            let resource_index =
                read_optional_json_mounted(CACHE_INDEX_PATH).unwrap_or_else(ResourceIndex::default);
            let persistent_state = read_optional_json_mounted(DEVICE_STATE_PATH)
                .unwrap_or_else(PersistentDeviceState::default);
            let battery = BatteryStatus::unknown();
            let power_profile = PowerProfile::from(&battery);
            let due = profile_sync_due(
                power_profile,
                persistent_state.last_successful_sync_epoch_seconds,
                now_epoch_seconds,
            );
            let mut sync = EspDeviceCloudSync::new(peripherals.modem);
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
                Err(_) => return Ok(Err(EspDeviceRunOutcome::EpdInitError)),
            };
            let mut display = PackedFrameDisplay::new(MountedSdCardDisplayResourceReader, epd_bus);
            let cycle = run_device_cycle(
                DeviceCycleInput {
                    config,
                    snapshot,
                    resource_index,
                    persistent_state,
                    trigger,
                    now_epoch_seconds,
                    date,
                    rotation_slot: rotation_slot(now_epoch_seconds),
                    battery,
                    daily_sync_due: due,
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
        Ok(Err(_)) => EspDeviceRunReport {
            outcome: EspDeviceRunOutcome::StorageMountError,
            cycle: None,
            sleep_plan: None,
        },
        Err(_) => EspDeviceRunReport {
            outcome: EspDeviceRunOutcome::StorageMountError,
            cycle: None,
            sleep_plan: None,
        },
    }
}

#[cfg(target_os = "espidf")]
struct EspDeviceCloudSync {
    modem: Option<esp_idf_svc::hal::modem::Modem<'static>>,
    wifi: Option<ConnectedWifi>,
    inner: CloudResourceSync<EspIdfHttpClient, MountedSdCardResourceStore>,
}

#[cfg(target_os = "espidf")]
impl EspDeviceCloudSync {
    fn new(modem: esp_idf_svc::hal::modem::Modem<'static>) -> Self {
        Self {
            modem: Some(modem),
            wifi: None,
            inner: CloudResourceSync::new(EspIdfHttpClient, MountedSdCardResourceStore),
        }
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
        if self.wifi.is_none() {
            let modem = self
                .modem
                .take()
                .ok_or(EspDeviceSyncError::Wifi(WifiConnectError::InitError))?;
            let wifi = connect_wifi(modem, &request.config).map_err(EspDeviceSyncError::Wifi)?;
            self.wifi = Some(wifi);
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
    if let Some(snapshot) = &cycle.snapshot {
        write_json_checked(PLANS_CURRENT_PATH, snapshot)?;
    }
    write_json_checked(CACHE_INDEX_PATH, &cycle.resource_index)?;
    write_json_checked(DEVICE_STATE_PATH, &cycle.persistent_state)?;
    write_json_checked(DISPLAY_STATE_PATH, &cycle.persistent_state.display_state())?;
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
    let next_sync = next_profile_sync_epoch_seconds(
        power_profile,
        cycle.persistent_state.last_successful_sync_epoch_seconds,
        now_epoch_seconds,
    )
    .unwrap_or(now_epoch_seconds.saturating_add(DAILY_SYNC_INTERVAL_SECONDS));
    let next_plan_change = cycle
        .snapshot
        .as_ref()
        .and_then(|snapshot| next_plan_change_date(&snapshot.plans, date))
        .map(local_date_start_epoch_seconds);

    next_wakeup_sleep_plan(now_epoch_seconds, next_sync, next_plan_change, None)
}

#[cfg(target_os = "espidf")]
fn now_epoch_seconds() -> u64 {
    chrono::Utc::now().timestamp().max(0) as u64
}

#[cfg(target_os = "espidf")]
fn today() -> LocalDate {
    use chrono::{Datelike, Local};

    let now = Local::now();
    LocalDate::new(now.year() as u16, now.month() as u8, now.day() as u8)
        .unwrap_or_else(|| LocalDate::parse("2026-01-01").unwrap())
}

#[cfg(target_os = "espidf")]
fn rotation_slot(now_epoch_seconds: u64) -> u64 {
    now_epoch_seconds / DAILY_SYNC_INTERVAL_SECONDS
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
