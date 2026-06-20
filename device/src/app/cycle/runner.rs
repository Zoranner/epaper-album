use super::{
    decide_display, decide_sync, DeviceCloudSync, DeviceDisplay, DisplayAction, DisplayCause,
    DisplayDecision, DisplayRefreshRequest, DisplayTarget, ErrorRefreshRequest, RunContext,
    SpriteSet, SyncAction, SyncDecision, SyncErrorReport, SyncRequest,
};
use crate::app::RunTrigger;
use crate::config::Config;
use crate::model::{LocalDate, Plan};
use crate::power::{BatteryStatus, PowerProfile};
use crate::state::{PersistentDeviceState, PersistentSyncState, RefreshReason};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceCycleInput {
    pub config: Option<Config>,
    pub plans: Option<Vec<Plan>>,
    pub persistent_state: PersistentDeviceState,
    pub persistent_state_loaded: bool,
    pub sync_state: PersistentSyncState,
    pub trigger: RunTrigger,
    pub now_epoch_seconds: u64,
    pub date: LocalDate,
    pub battery: BatteryStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceCycleResult {
    pub plans: Option<Vec<Plan>>,
    pub persistent_state: PersistentDeviceState,
    pub sync_state: PersistentSyncState,
    pub battery: BatteryStatus,
    pub sync_decision: SyncDecision,
    pub display_decision: DisplayDecision,
    pub outcome: DeviceCycleOutcome,
    pub sync_attempted: bool,
    pub sync_succeeded: bool,
    pub sync_error: Option<String>,
    pub sync_error_report: Option<SyncErrorReport>,
    pub refresh_attempted: bool,
    pub refresh_succeeded: bool,
    pub display_available: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceCycleOutcome {
    SleepOnly,
    SyncRequested,
    RefreshOnly,
    LowBatterySkipSync,
    MissingConfig,
    NoUsablePhoto,
    SyncFailed,
    RefreshFailed,
}

pub fn run_device_cycle<S, D>(
    input: DeviceCycleInput,
    sync: &mut S,
    display: &mut D,
) -> DeviceCycleResult
where
    S: DeviceCloudSync,
    D: DeviceDisplay,
{
    let DeviceCycleInput {
        config,
        mut plans,
        mut persistent_state,
        persistent_state_loaded,
        mut sync_state,
        trigger,
        now_epoch_seconds,
        date,
        battery,
    } = input;

    let sync_decision = decide_sync(config.as_ref(), &battery, &sync_state, date);
    let sync_requested = sync_decision.action == SyncAction::Fetch;
    let effective_low_battery = battery.effective_low_battery();
    let mut sync_attempted = false;
    let mut sync_succeeded = false;
    let mut sync_error = None;
    let mut sync_error_report = None;
    let mut sync_failed = false;
    let mut sprites = SpriteSet::default();

    if sync_requested && !effective_low_battery {
        if let Some(config) = config
            .as_ref()
            .filter(|config| config.has_required_values())
        {
            sync_attempted = true;
            let request = SyncRequest {
                config: config.clone(),
                local_plans: plans.clone(),
                date,
                now_epoch_seconds,
            };

            match sync.sync_resources(request) {
                Ok(sync_result) => {
                    plans = Some(sync_result.plans);
                    sprites = sync_result.sprites;
                    sync_succeeded = true;
                    sync_state.date = Some(date);
                }
                Err(error) => {
                    let report = sync.describe_error(&error);
                    sync_error = Some(error.to_string());
                    sync_error_report = Some(report);
                    sync_failed = true;
                }
            }
        } else {
            sync_error = Some("missing or incomplete config".to_string());
            sync_error_report = Some(SyncErrorReport::new(
                "config.missing",
                "config",
                None,
                "DEVICE CONFIG IS MISSING",
                "missing or incomplete config",
            ));
        }
    }

    let context = RunContext {
        now: now_epoch_seconds,
        date,
        trigger,
        battery,
        power: PowerProfile::from(&battery),
        config: config.clone(),
        plans: plans.clone(),
        state: persistent_state.clone(),
        sync: sync_state.clone(),
    };
    let decision = decide_display(
        &context,
        |sha256| display.has_image(sha256),
        sync_error_report.as_ref(),
    );

    let mut refresh_attempted = false;
    let mut refresh_succeeded = false;
    let mut refresh_failed = false;
    if let DisplayAction::Refresh(target) = &decision.action {
        refresh_attempted = true;
        let result = refresh_target(
            display,
            &mut persistent_state,
            target.clone(),
            sprites.clone(),
            now_epoch_seconds,
            decision.cause,
        );
        match result {
            Ok(()) => refresh_succeeded = true,
            Err(error) => {
                log::warn!(target: "epaper_album", "refresh: {error}");
                sync_error = Some(error.to_string());
                sync_error_report = Some(SyncErrorReport::new(
                    "display.refresh",
                    "display",
                    None,
                    "DISPLAY REFRESH FAILED",
                    error.to_string(),
                ));
                refresh_failed = true;
            }
        }
    }
    let display_available = refresh_succeeded
        || (persistent_state_loaded && matches!(decision.action, DisplayAction::Keep));

    let outcome = cycle_outcome(
        &decision,
        config.as_ref(),
        sync_requested,
        effective_low_battery,
        sync_failed,
        refresh_failed,
        refresh_attempted,
    );

    DeviceCycleResult {
        plans,
        persistent_state,
        sync_state,
        battery,
        sync_decision,
        display_decision: decision,
        outcome,
        sync_attempted,
        sync_succeeded,
        sync_error,
        sync_error_report,
        refresh_attempted,
        refresh_succeeded,
        display_available,
    }
}

fn photo_refresh_request(
    plan: Plan,
    date: LocalDate,
    reason: RefreshReason,
    sprites: SpriteSet,
    now_epoch_seconds: u64,
) -> DisplayRefreshRequest {
    DisplayRefreshRequest {
        plan,
        date,
        reason,
        sprites,
        now_epoch_seconds,
    }
}

fn refresh_photo<D>(
    display: &mut D,
    persistent_state: &mut PersistentDeviceState,
    request: DisplayRefreshRequest,
) -> Result<(), D::Error>
where
    D: DeviceDisplay,
{
    let plan = request.plan.clone();
    let date = request.date;

    display.refresh(request)?;
    persistent_state.set_display(date, &plan);
    Ok(())
}

fn refresh_target<D>(
    display: &mut D,
    persistent_state: &mut PersistentDeviceState,
    target: DisplayTarget,
    sprites: SpriteSet,
    now_epoch_seconds: u64,
    cause: DisplayCause,
) -> Result<(), D::Error>
where
    D: DeviceDisplay,
{
    match target {
        DisplayTarget::Photo {
            date,
            image,
            caption,
        } => {
            let plan = Plan::fixed(date, caption, image);
            let request = photo_refresh_request(
                plan,
                date,
                refresh_reason(cause),
                sprites,
                now_epoch_seconds,
            );
            refresh_photo(display, persistent_state, request)
        }
        DisplayTarget::Page {
            date,
            title,
            message,
            hint,
            detail,
        } => {
            display.refresh_error_page(ErrorRefreshRequest {
                title,
                message,
                hint,
                detail,
                now_epoch_seconds,
            })?;
            persistent_state.set_page(date);
            Ok(())
        }
    }
}

const fn refresh_reason(cause: DisplayCause) -> RefreshReason {
    match cause {
        DisplayCause::First => RefreshReason::FirstBoot,
        DisplayCause::Date | DisplayCause::Photo => RefreshReason::PlanChanged,
        DisplayCause::LowBattery
        | DisplayCause::Sync
        | DisplayCause::MissingConfig
        | DisplayCause::MissingPhoto
        | DisplayCause::Same => RefreshReason::ErrorPage,
    }
}

fn cycle_outcome(
    decision: &DisplayDecision,
    config: Option<&Config>,
    sync_requested: bool,
    low_battery: bool,
    sync_failed: bool,
    refresh_failed: bool,
    refresh_attempted: bool,
) -> DeviceCycleOutcome {
    if config.is_none_or(|config| !config.has_required_values()) {
        return DeviceCycleOutcome::MissingConfig;
    }

    if low_battery {
        return DeviceCycleOutcome::LowBatterySkipSync;
    }

    if refresh_failed {
        return DeviceCycleOutcome::RefreshFailed;
    }

    if sync_failed {
        return DeviceCycleOutcome::SyncFailed;
    }

    match decision.cause {
        DisplayCause::MissingConfig => DeviceCycleOutcome::MissingConfig,
        DisplayCause::MissingPhoto => DeviceCycleOutcome::NoUsablePhoto,
        _ if matches!(decision.action, DisplayAction::Refresh(_)) || refresh_attempted => {
            DeviceCycleOutcome::RefreshOnly
        }
        _ if sync_requested => DeviceCycleOutcome::SyncRequested,
        _ => DeviceCycleOutcome::SleepOnly,
    }
}
