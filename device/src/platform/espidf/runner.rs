use super::diagnostics::MountedDiagnosticLog;
use super::error_page::refresh_storage_error_page;
use super::schedule::{build_next_run_plan, current_epoch_seconds, today};
use super::storage::{read_config_mounted, read_optional_json_mounted, write_cycle_files};
use super::sync::EspDeviceCloudSync;
use super::{EspDeviceRunOutcome, EspDeviceRunReport};
use crate::app::RunTrigger;
use crate::device_output::{MountedSdCardDisplayResourceReader, PackedFrameDisplay};
use crate::device_runtime::{decide_sync, run_device_cycle, DeviceCycleInput, SyncAction};
use crate::epd::espidf::EspEpdBus;
use crate::pmic::espidf::{init_axp2101_for_photo_painter, status_summary};
use crate::power::{BatteryStatus, PowerProfile};
use crate::state::{PersistentDeviceState, PersistentSyncState};
use crate::storage::{with_mounted_sdcard_parts, PLAN_PATH, STATE_PATH, SYNC_PATH};

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
                target: "inkframe_device",
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
            log::warn!(target: "inkframe_device", "pmic: init-error: {error:?}");
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
                    sync.prepare_network(config);
                    if sync.time_synced() {
                        now_epoch_seconds = current_epoch_seconds();
                        date = today();
                        log::info!(
                            target: "inkframe_device",
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
                target: "inkframe_device",
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
            let next_run_plan = build_next_run_plan(&cycle, now_epoch_seconds);
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
