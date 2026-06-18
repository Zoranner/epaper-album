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
    decide_sync, run_device_cycle, DeviceCloudSync, DeviceCycleInput, DeviceCycleOutcome,
    DeviceCycleResult, DeviceDisplay, DisplayAction, DisplayTarget, ErrorRefreshRequest,
    SyncAction, SyncErrorReport, SyncRequest, SyncResult,
};
#[cfg(target_os = "espidf")]
use crate::device_sync::{CloudResourceSync, DeviceSyncError};
#[cfg(target_os = "espidf")]
use crate::diagnostics::{
    append_event_to_file, daily_log_path, remove_logs_older_than, DiagnosticEvent, DiagnosticLevel,
    DiagnosticLogWrite, LOGS_DIR,
};
#[cfg(target_os = "espidf")]
use crate::epd::espidf::EspEpdBus;
#[cfg(target_os = "espidf")]
use crate::model::LocalDate;
#[cfg(target_os = "espidf")]
use crate::pmic::espidf::{init_axp2101_for_photo_painter, status_summary};
#[cfg(target_os = "espidf")]
use crate::power::{
    next_power_run_epoch_seconds, next_run_plan, BatteryStatus, NextRunPlan, PowerProfile,
};
#[cfg(target_os = "espidf")]
use crate::state::{PersistentDeviceState, PersistentSyncState};
#[cfg(target_os = "espidf")]
use crate::storage::{
    read_json_file_mounted, read_text_file_mounted, with_mounted_sdcard_parts,
    write_json_file_atomic_mounted, MountedSdCardResourceStore, StorageJsonRead, StorageJsonWrite,
    StorageRead, PLAN_PATH, STATE_PATH, SYNC_PATH,
};
#[cfg(target_os = "espidf")]
use crate::wifi::espidf::{connect_wifi, ConnectedWifi, WifiConnectError};
#[cfg(target_os = "espidf")]
use core::time::Duration;

#[cfg(target_os = "espidf")]
const DIAGNOSTIC_LOG_KEEP_DAYS: u8 = 14;

#[cfg(target_os = "espidf")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspDeviceRunReport {
    pub outcome: EspDeviceRunOutcome,
    pub cycle: Option<DeviceCycleResult>,
    pub next_run_plan: Option<NextRunPlan>,
}

#[cfg(target_os = "espidf")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EspDeviceRunOutcome {
    Completed(DeviceCycleOutcome),
    SelfTest,
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
            Self::SelfTest => "self-test",
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
                next_run_plan: None,
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
                next_run_plan: None,
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
            let run_epoch_seconds = now_epoch_seconds;
            let mut diagnostics = MountedDiagnosticLog::new(date, run_epoch_seconds);
            diagnostics.info(
                now_epoch_seconds,
                "trigger",
                "device cycle started",
                |event| event.with_data("trigger", format!("{trigger:?}")),
            );
            let config = read_config_mounted();
            let plans = read_optional_json_mounted(PLAN_PATH);
            let loaded_persistent_state = read_optional_json_mounted(STATE_PATH);
            let persistent_state_loaded = loaded_persistent_state.is_some();
            let persistent_state =
                loaded_persistent_state.unwrap_or_else(PersistentDeviceState::default);
            let sync_state =
                read_optional_json_mounted(SYNC_PATH).unwrap_or_else(PersistentSyncState::default);
            let battery = pmic_probe
                .map(|probe| probe.battery)
                .unwrap_or_else(BatteryStatus::unknown);
            let mut sync = EspDeviceCloudSync::new(peripherals.modem);
            let pre_sync_decision = decide_sync(config.as_ref(), &battery, &sync_state, date);
            if pre_sync_decision.action == SyncAction::Fetch {
                if let Some(config) = config
                    .as_ref()
                    .filter(|config| config.has_required_values())
                {
                    // This preflight only decides whether time sync is worth attempting.
                    // The final sync decision is recalculated inside run_device_cycle.
                    sync.prepare_network(config);
                    if sync.time_synced {
                        now_epoch_seconds = current_epoch_seconds();
                        date = today();
                        log::info!(
                            target: "epaper_album",
                            "time: unix={} date={}",
                            now_epoch_seconds,
                            date
                        );
                        diagnostics = MountedDiagnosticLog::new(date, run_epoch_seconds);
                        diagnostics.info(now_epoch_seconds, "time", "time synchronized", |event| {
                            event
                                .with_data("unix", now_epoch_seconds)
                                .with_data("date", date.to_string())
                        });
                    }
                }
            }

            let power_profile = PowerProfile::from(&battery);
            log::info!(
                target: "epaper_album",
                "power: profile={:?} run-interval={:?} battery={:?} percent={:?} low={}",
                power_profile,
                power_profile.run_interval_seconds(),
                battery.charge_state,
                battery.percent,
                battery.low_battery
            );
            diagnostics.info(
                now_epoch_seconds,
                "power",
                "power profile resolved",
                |event| {
                    event
                        .with_data("profile", format!("{power_profile:?}"))
                        .with_data("interval", power_profile.run_interval_seconds())
                        .with_data("battery", format!("{:?}", battery.charge_state))
                        .with_data("low", battery.low_battery)
                        .with_data(
                            "percent",
                            battery
                                .percent
                                .map_or(serde_json::Value::Null, serde_json::Value::from),
                        )
                },
            );
            let cycle = run_device_cycle(
                DeviceCycleInput {
                    config,
                    plans,
                    persistent_state,
                    persistent_state_loaded,
                    sync_state,
                    trigger,
                    now_epoch_seconds,
                    date,
                    battery,
                },
                &mut sync,
                &mut display,
            );

            if let Err(outcome) = write_cycle_files(&cycle) {
                diagnostics.error(now_epoch_seconds, "state", "state write failed", |event| {
                    event.with_data("outcome", outcome.label())
                });
                return Ok(Err(outcome));
            }
            let next_run_plan = build_next_run_plan(&cycle, now_epoch_seconds, date);
            diagnostics.record_cycle(now_epoch_seconds, &cycle, &next_run_plan);
            Ok(Ok((cycle, next_run_plan)))
        },
    );

    match result {
        Ok(Ok(Ok((cycle, next_run_plan)))) => EspDeviceRunReport {
            outcome: EspDeviceRunOutcome::Completed(cycle.outcome.clone()),
            cycle: Some(cycle),
            next_run_plan: Some(next_run_plan),
        },
        Ok(Ok(Err(outcome))) => EspDeviceRunReport {
            outcome,
            cycle: None,
            next_run_plan: None,
        },
        Ok(Err(_)) | Err(_) => {
            refresh_storage_error_page(&mut display, now_epoch_seconds);
            EspDeviceRunReport {
                outcome: EspDeviceRunOutcome::StorageMountError,
                cycle: None,
                next_run_plan: None,
            }
        }
    }
}

#[cfg(target_os = "espidf")]
struct MountedDiagnosticLog {
    path: std::path::PathBuf,
    run_epoch_seconds: u64,
}

#[cfg(target_os = "espidf")]
impl MountedDiagnosticLog {
    fn new(date: LocalDate, run_epoch_seconds: u64) -> Self {
        let _ = remove_logs_older_than(LOGS_DIR, date, DIAGNOSTIC_LOG_KEEP_DAYS);
        Self {
            path: daily_log_path(date),
            run_epoch_seconds,
        }
    }

    fn info(
        &mut self,
        time: u64,
        event: &str,
        message: &str,
        data: impl FnOnce(DiagnosticEvent) -> DiagnosticEvent,
    ) {
        self.write(data(DiagnosticEvent::new(
            time,
            self.run_epoch_seconds,
            DiagnosticLevel::Info,
            event,
            message,
        )));
    }

    fn warn(
        &mut self,
        time: u64,
        event: &str,
        message: &str,
        data: impl FnOnce(DiagnosticEvent) -> DiagnosticEvent,
    ) {
        self.write(data(DiagnosticEvent::new(
            time,
            self.run_epoch_seconds,
            DiagnosticLevel::Warn,
            event,
            message,
        )));
    }

    fn error(
        &mut self,
        time: u64,
        event: &str,
        message: &str,
        data: impl FnOnce(DiagnosticEvent) -> DiagnosticEvent,
    ) {
        self.write(data(DiagnosticEvent::new(
            time,
            self.run_epoch_seconds,
            DiagnosticLevel::Error,
            event,
            message,
        )));
    }

    fn record_cycle(&mut self, time: u64, cycle: &DeviceCycleResult, next_run_plan: &NextRunPlan) {
        self.info(time, "cycle", "device cycle completed", |event| {
            event.with_data("outcome", format!("{:?}", cycle.outcome))
        });
        self.info(time, "sync", "sync decision resolved", |event| {
            let event = event
                .with_data("action", format!("{:?}", cycle.sync_decision.action))
                .with_data("cause", format!("{:?}", cycle.sync_decision.cause))
                .with_data("attempted", cycle.sync_attempted)
                .with_data("succeeded", cycle.sync_succeeded);
            append_sync_error_data(event, cycle)
        });
        if cycle.sync_error.is_some() {
            self.warn(time, "sync", "sync failed", |event| {
                append_sync_error_data(event, cycle)
            });
        }
        self.info(time, "display", "display decision resolved", |event| {
            append_display_decision_data(event, &cycle.display_decision)
                .with_data("refresh_attempted", cycle.refresh_attempted)
                .with_data("refresh_succeeded", cycle.refresh_succeeded)
        });
        self.info(time, "next", "next run scheduled", |event| {
            event
                .with_data("at", next_run_plan.next_run_epoch_seconds)
                .with_data("wait", next_run_plan.wait_seconds)
                .with_data(
                    "mode",
                    if cycle.battery.externally_powered() {
                        "restart"
                    } else {
                        "deep-sleep"
                    },
                )
        });
    }

    fn write(&self, event: DiagnosticEvent) {
        match append_event_to_file(&self.path, &event) {
            DiagnosticLogWrite::Written => {}
            error => {
                log::warn!(target: "epaper_album", "diagnostic log write failed: {error:?}");
            }
        }
    }
}

#[cfg(target_os = "espidf")]
fn append_sync_error_data(
    event: DiagnosticEvent,
    cycle: &crate::device_runtime::DeviceCycleResult,
) -> DiagnosticEvent {
    let Some(error) = cycle.sync_error.as_ref() else {
        return event;
    };
    let event = event.with_data("error", error.to_string());
    let Some(report) = cycle.sync_error_report.as_ref() else {
        return event;
    };
    let event = event
        .with_data("code", report.code.clone())
        .with_data("category", report.category.clone())
        .with_data("message", report.message.clone())
        .with_data("detail", report.detail.clone());
    match &report.stage {
        Some(stage) => event.with_data("stage", stage.clone()),
        None => event,
    }
}

#[cfg(target_os = "espidf")]
fn append_display_decision_data(
    event: DiagnosticEvent,
    decision: &crate::device_runtime::DisplayDecision,
) -> DiagnosticEvent {
    match &decision.action {
        DisplayAction::Keep => event
            .with_data("action", "Keep")
            .with_data("cause", format!("{:?}", decision.cause)),
        DisplayAction::Refresh(DisplayTarget::Photo {
            date,
            image,
            caption,
        }) => event
            .with_data("action", "RefreshPhoto")
            .with_data("cause", format!("{:?}", decision.cause))
            .with_data("date", date.to_string())
            .with_data("image", image.clone())
            .with_data("caption", caption.clone()),
        DisplayAction::Refresh(DisplayTarget::Page {
            date,
            title,
            message,
            hint,
            detail,
        }) => event
            .with_data("action", "RefreshPage")
            .with_data("cause", format!("{:?}", decision.cause))
            .with_data("date", date.to_string())
            .with_data("title", title.clone())
            .with_data("message", message.clone())
            .with_data("hint", hint.clone())
            .with_data("detail", detail.clone()),
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
impl EspDeviceSyncError {
    fn code(&self) -> String {
        match self {
            Self::Wifi(error) => format!("wifi.{}", wifi_error_code(*error)),
            Self::Sync(error) => error.code(),
        }
    }

    const fn category(&self) -> &'static str {
        match self {
            Self::Wifi(_) => "wifi",
            Self::Sync(error) => error.category(),
        }
    }

    fn stage(&self) -> Option<String> {
        match self {
            Self::Wifi(_) => Some("wifi".to_string()),
            Self::Sync(error) => Some(error.stage().to_string()),
        }
    }

    fn message(&self) -> String {
        match self {
            Self::Wifi(_) => "wifi connection failed".to_string(),
            Self::Sync(error) => error.message(),
        }
    }

    fn detail(&self) -> String {
        match self {
            Self::Wifi(error) => format!("{error:?}"),
            Self::Sync(error) => error.detail().unwrap_or_else(|| error.to_string()),
        }
    }
}

#[cfg(target_os = "espidf")]
const fn wifi_error_code(error: WifiConnectError) -> &'static str {
    match error {
        WifiConnectError::InitError => "init",
        WifiConnectError::ConfigError => "config",
        WifiConnectError::StartError => "start",
        WifiConnectError::ScanError => "scan",
        WifiConnectError::TargetNotFound => "target-not-found",
        WifiConnectError::ConnectError => "connect",
        WifiConnectError::NetifError => "netif",
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
        log::info!(
            target: "epaper_album",
            "sync: network ready date={}",
            request.date
        );
        if let Some(wifi) = self.wifi.as_ref() {
            match wifi.ip_info() {
                Ok(ip_info) => log::info!(
                    target: "epaper_album",
                    "sync: ip={} netmask={} dns={:?} secondary-dns={:?}",
                    ip_info.ip,
                    ip_info.subnet.mask,
                    ip_info.dns,
                    ip_info.secondary_dns
                ),
                Err(error) => log::warn!(target: "epaper_album", "sync: ip-info error: {error:?}"),
            }
        }

        self.inner
            .sync_resources(request)
            .map_err(EspDeviceSyncError::Sync)
    }

    fn describe_error(&self, error: &Self::Error) -> SyncErrorReport {
        SyncErrorReport::new(
            error.code(),
            error.category(),
            error.stage(),
            error.message(),
            error.detail(),
        )
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
    write_json_checked(SYNC_PATH, &cycle.sync_state)?;
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
fn build_next_run_plan(
    cycle: &DeviceCycleResult,
    now_epoch_seconds: u64,
    _date: LocalDate,
) -> NextRunPlan {
    let power_profile = PowerProfile::from(&cycle.battery);
    let next_power_run = next_power_run_epoch_seconds(now_epoch_seconds, power_profile);

    next_run_plan(now_epoch_seconds, next_power_run, None, None)
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
