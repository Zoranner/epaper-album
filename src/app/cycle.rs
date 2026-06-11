use crate::app::{
    generate_display_decision, DisplayDecision, NoUsablePhotoReason, RunOutcome, RunTrigger,
};
use crate::config::Config;
use crate::model::{LocalDate, Plan};
use crate::power::BatteryStatus;
use crate::render::RenderNotice;
use crate::state::{PersistentDeviceState, RefreshReason};
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceCycleInput {
    pub config: Option<Config>,
    pub plans: Option<Vec<Plan>>,
    pub persistent_state: PersistentDeviceState,
    pub trigger: RunTrigger,
    pub now_epoch_seconds: u64,
    pub date: LocalDate,
    pub battery: BatteryStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncRequest {
    pub config: Config,
    pub local_plans: Option<Vec<Plan>>,
    pub notice: Option<RenderNotice>,
    pub now_epoch_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncResult {
    pub plans: Vec<Plan>,
    pub sprites: SpriteSet,
    pub sprites_changed: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SpriteSet {
    pub caption: Option<String>,
    pub date: Option<String>,
    pub notice: Option<String>,
}

pub trait DeviceCloudSync {
    type Error: fmt::Display;

    fn sync_resources(&mut self, request: SyncRequest) -> Result<SyncResult, Self::Error>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisplayRefreshRequest {
    pub plan: Plan,
    pub reason: RefreshReason,
    pub notice: Option<RenderNotice>,
    pub sprites: SpriteSet,
    pub now_epoch_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ErrorRefreshRequest {
    pub title: String,
    pub message: String,
    pub hint: String,
    pub detail: String,
    pub now_epoch_seconds: u64,
}

pub trait DeviceDisplay {
    type Error: fmt::Display;

    fn refresh(&mut self, request: DisplayRefreshRequest) -> Result<(), Self::Error>;
    fn refresh_error_page(&mut self, request: ErrorRefreshRequest) -> Result<(), Self::Error>;
    fn has_image(&self, sha256: &str) -> bool;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceCycleResult {
    pub plans: Option<Vec<Plan>>,
    pub persistent_state: PersistentDeviceState,
    pub battery: BatteryStatus,
    pub display_decision: DisplayDecision,
    pub outcome: DeviceCycleOutcome,
    pub sync_attempted: bool,
    pub sync_succeeded: bool,
    pub sync_error: Option<String>,
    pub refresh_attempted: bool,
    pub refresh_succeeded: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeviceCycleOutcome {
    MissingConfig,
    SyncRequested,
    RefreshOnly,
    SleepOnly,
    LowBatterySkipSync,
    SyncFailed,
    RefreshFailed,
    NoUsablePhoto(NoUsablePhotoReason),
}

impl From<RunOutcome> for DeviceCycleOutcome {
    fn from(outcome: RunOutcome) -> Self {
        match outcome {
            RunOutcome::SyncRequested => Self::SyncRequested,
            RunOutcome::RefreshOnly => Self::RefreshOnly,
            RunOutcome::SleepOnly => Self::SleepOnly,
            RunOutcome::LowBatterySkipSync => Self::LowBatterySkipSync,
        }
    }
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
        trigger: _,
        now_epoch_seconds,
        date,
        battery,
    } = input;

    let sync_requested = true;
    let effective_low_battery = battery.effective_low_battery();
    let previous_notice = persistent_state.notice;
    let mut sync_attempted = false;
    let mut sync_succeeded = false;
    let mut sync_error = None;
    let mut sync_failed = false;
    let mut sprites = SpriteSet::default();
    let mut sprites_changed = false;

    if sync_requested && !effective_low_battery {
        if let Some(config) = config
            .as_ref()
            .filter(|config| config.has_required_values())
        {
            sync_attempted = true;
            let request = SyncRequest {
                config: config.clone(),
                local_plans: plans.clone(),
                notice: refresh_notice(effective_low_battery, false),
                now_epoch_seconds,
            };

            match sync.sync_resources(request) {
                Ok(sync_result) => {
                    plans = Some(sync_result.plans);
                    sprites = sync_result.sprites;
                    sprites_changed = sync_result.sprites_changed;
                    sync_succeeded = true;
                }
                Err(error) => {
                    sync_error = Some(error.to_string());
                    sync_failed = true;
                }
            }
        } else {
            sync_error = Some("missing or incomplete config".to_string());
        }
    }

    let decision = generate_display_decision(
        plans.as_deref(),
        |sha256| display.has_image(sha256),
        date,
        Some(&persistent_state),
    );

    let mut refresh_attempted = false;
    let mut refresh_succeeded = false;
    let mut refresh_failed = false;
    let low_battery_refresh = effective_low_battery
        .then(|| low_battery_refresh_for_notice(display, &persistent_state))
        .flatten();
    let recovered_notice_refresh = (!effective_low_battery
        && previous_notice.is_some()
        && matches!(decision, DisplayDecision::SleepOnly { .. }))
    .then(|| fallback_refresh_for_notice(plans.as_deref(), display, date))
    .flatten();

    let sync_failure_fallback = (low_battery_refresh.is_none()
        && recovered_notice_refresh.is_none()
        && sync_failed
        && matches!(decision, DisplayDecision::SleepOnly { .. }))
    .then(|| fallback_refresh_for_notice(plans.as_deref(), display, date))
    .flatten();
    let overlay_refresh = (low_battery_refresh.is_none()
        && recovered_notice_refresh.is_none()
        && sync_succeeded
        && sprites_changed
        && matches!(decision, DisplayDecision::SleepOnly { .. }))
    .then(|| fallback_refresh_for_notice(plans.as_deref(), display, date))
    .flatten();

    if let Some((plan, reason)) = low_battery_refresh {
        refresh_attempted = true;
        let request = photo_refresh_request(
            plan,
            reason,
            Some(RenderNotice::LowBattery),
            SpriteSet::default(),
            now_epoch_seconds,
        );
        match refresh_photo(display, &mut persistent_state, request) {
            Ok(()) => refresh_succeeded = true,
            Err(error) => {
                log::warn!(target: "epaper_album", "refresh: {error}");
                sync_error = Some(error.to_string());
                refresh_failed = true;
            }
        }
    } else if let Some((plan, reason)) = recovered_notice_refresh {
        refresh_attempted = true;
        let request = photo_refresh_request(plan, reason, None, sprites.clone(), now_epoch_seconds);
        match refresh_photo(display, &mut persistent_state, request) {
            Ok(()) => refresh_succeeded = true,
            Err(error) => {
                log::warn!(target: "epaper_album", "refresh: {error}");
                sync_error = Some(error.to_string());
                refresh_failed = true;
            }
        }
    } else if let Some((plan, reason)) = sync_failure_fallback {
        refresh_attempted = true;
        let request = photo_refresh_request(
            plan,
            reason,
            refresh_notice(effective_low_battery, sync_failed),
            SpriteSet::default(),
            now_epoch_seconds,
        );
        match refresh_photo(display, &mut persistent_state, request) {
            Ok(()) => refresh_succeeded = true,
            Err(error) => {
                log::warn!(target: "epaper_album", "refresh: {error}");
                sync_error = Some(error.to_string());
                refresh_failed = true;
            }
        }
    } else if let Some((plan, reason)) = overlay_refresh {
        refresh_attempted = true;
        let request = photo_refresh_request(
            plan,
            reason,
            refresh_notice(effective_low_battery, sync_failed),
            sprites.clone(),
            now_epoch_seconds,
        );
        match refresh_photo(display, &mut persistent_state, request) {
            Ok(()) => refresh_succeeded = true,
            Err(error) => {
                log::warn!(target: "epaper_album", "refresh: {error}");
                sync_error = Some(error.to_string());
                refresh_failed = true;
            }
        }
    } else if let DisplayDecision::RefreshRequired { plan, reason } = &decision {
        refresh_attempted = true;
        let request = photo_refresh_request(
            plan.clone(),
            *reason,
            refresh_notice(effective_low_battery, sync_failed),
            sprites.clone(),
            now_epoch_seconds,
        );
        match refresh_photo(display, &mut persistent_state, request) {
            Ok(()) => refresh_succeeded = true,
            Err(error) => {
                log::warn!(target: "epaper_album", "refresh: {error}");
                sync_error = Some(error.to_string());
                refresh_failed = true;
            }
        }
    } else if let Some(request) = error_refresh_request(
        config.as_ref(),
        &decision,
        sync_error.as_deref(),
        sync_failed,
        now_epoch_seconds,
    ) {
        refresh_attempted = true;

        match display.refresh_error_page(request) {
            Ok(()) => refresh_succeeded = true,
            Err(error) => {
                log::warn!(target: "epaper_album", "error page refresh: {error}");
                sync_error = Some(error.to_string());
                refresh_failed = true;
            }
        }
    }

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
        battery,
        display_decision: decision,
        outcome,
        sync_attempted,
        sync_succeeded,
        sync_error,
        refresh_attempted,
        refresh_succeeded,
    }
}

fn refresh_notice(low_battery: bool, sync_failed: bool) -> Option<RenderNotice> {
    if low_battery {
        return Some(RenderNotice::LowBattery);
    }

    if sync_failed {
        return Some(RenderNotice::SyncFailed);
    }

    None
}

fn photo_refresh_request(
    plan: Plan,
    reason: RefreshReason,
    notice: Option<RenderNotice>,
    sprites: SpriteSet,
    now_epoch_seconds: u64,
) -> DisplayRefreshRequest {
    DisplayRefreshRequest {
        plan,
        reason,
        notice,
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
    let notice = request.notice;

    display.refresh(request)?;
    persistent_state.set(&plan, notice);
    Ok(())
}

fn low_battery_refresh_for_notice<D>(
    display: &D,
    persistent_state: &PersistentDeviceState,
) -> Option<(Plan, RefreshReason)>
where
    D: DeviceDisplay,
{
    if persistent_state.notice == Some(RenderNotice::LowBattery) {
        return None;
    }

    let plan = persistent_state.to_plan()?;
    if !display.has_image(&plan.image) {
        return None;
    }

    Some((plan, RefreshReason::NoticeChanged))
}

fn fallback_refresh_for_notice<D>(
    plans: Option<&[Plan]>,
    display: &D,
    date: LocalDate,
) -> Option<(Plan, RefreshReason)>
where
    D: DeviceDisplay,
{
    let plan = crate::schedule::select_plan_for_date(plans?, date)?;
    display
        .has_image(&plan.image)
        .then(|| (plan.clone(), RefreshReason::NoticeChanged))
}

fn error_refresh_request(
    config: Option<&Config>,
    decision: &DisplayDecision,
    sync_error: Option<&str>,
    sync_failed: bool,
    now_epoch_seconds: u64,
) -> Option<ErrorRefreshRequest> {
    if config.is_none_or(|config| !config.has_required_values()) {
        return Some(ErrorRefreshRequest {
            title: "CONFIG ERROR".to_string(),
            message: "DEVICE CONFIG IS MISSING".to_string(),
            hint: "CHECK /SDCARD/CONFIG.TOML".to_string(),
            detail: "WIFI BASE URL AND SECRET KEY REQUIRED".to_string(),
            now_epoch_seconds,
        });
    }

    match decision {
        DisplayDecision::MissingConfig => {
            if sync_failed {
                return Some(ErrorRefreshRequest {
                    title: "SYNC ERROR".to_string(),
                    message: "CANNOT UPDATE SERVER DATA".to_string(),
                    hint: "CHECK WIFI BASE URL AND SERVER".to_string(),
                    detail: sync_error.unwrap_or("SYNC FAILED").to_string(),
                    now_epoch_seconds,
                });
            }

            Some(ErrorRefreshRequest {
                title: "CONFIG ERROR".to_string(),
                message: "DEVICE CONFIG IS MISSING".to_string(),
                hint: "CHECK /SDCARD/CONFIG.TOML".to_string(),
                detail: "WIFI BASE URL AND SECRET KEY REQUIRED".to_string(),
                now_epoch_seconds,
            })
        }
        DisplayDecision::NoUsablePhoto(reason) => {
            if sync_failed {
                return Some(ErrorRefreshRequest {
                    title: "SYNC ERROR".to_string(),
                    message: "CANNOT UPDATE SERVER DATA".to_string(),
                    hint: "CHECK WIFI BASE URL AND SECRET KEY".to_string(),
                    detail: sync_error.unwrap_or("SYNC FAILED").to_string(),
                    now_epoch_seconds,
                });
            }

            let (message, detail) = match reason {
                NoUsablePhotoReason::NoPlan => ("NO PLAN", "WAITING FOR SERVER PLAN"),
                NoUsablePhotoReason::ResourceNotCached => {
                    ("PLANNED IMAGE IS NOT READY", "IMAGE CACHE IS MISSING")
                }
            };

            Some(ErrorRefreshRequest {
                title: "NO PHOTO".to_string(),
                message: message.to_string(),
                hint: "CHECK SERVER PLAN AND IMAGE CACHE".to_string(),
                detail: detail.to_string(),
                now_epoch_seconds,
            })
        }
        DisplayDecision::RefreshRequired { .. } | DisplayDecision::SleepOnly { .. } => None,
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

    if sync_requested && low_battery {
        return DeviceCycleOutcome::LowBatterySkipSync;
    }

    if refresh_failed {
        return DeviceCycleOutcome::RefreshFailed;
    }

    if let DisplayDecision::NoUsablePhoto(reason) = decision {
        return DeviceCycleOutcome::NoUsablePhoto(reason.clone());
    }

    if sync_failed {
        return DeviceCycleOutcome::SyncFailed;
    }

    match decision {
        DisplayDecision::MissingConfig => DeviceCycleOutcome::MissingConfig,
        DisplayDecision::NoUsablePhoto(reason) => DeviceCycleOutcome::NoUsablePhoto(reason.clone()),
        DisplayDecision::RefreshRequired { .. } if refresh_attempted => {
            DeviceCycleOutcome::RefreshOnly
        }
        DisplayDecision::RefreshRequired { .. } => DeviceCycleOutcome::RefreshOnly,
        DisplayDecision::SleepOnly { .. } if sync_requested => DeviceCycleOutcome::SyncRequested,
        DisplayDecision::SleepOnly { .. } => DeviceCycleOutcome::SleepOnly,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::WakeReason;

    #[derive(Debug, Clone, Eq, PartialEq)]
    struct FakeError(&'static str);

    impl fmt::Display for FakeError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(self.0)
        }
    }

    #[derive(Default)]
    struct FakeSync {
        result: Option<Result<SyncResult, FakeError>>,
        requests: Vec<SyncRequest>,
    }

    impl DeviceCloudSync for FakeSync {
        type Error = FakeError;

        fn sync_resources(&mut self, request: SyncRequest) -> Result<SyncResult, Self::Error> {
            self.requests.push(request);
            self.result
                .take()
                .unwrap_or(Err(FakeError("unexpected sync")))
        }
    }

    #[derive(Default)]
    struct FakeDisplay {
        images: Vec<String>,
        result: Option<Result<(), FakeError>>,
        requests: Vec<DisplayRefreshRequest>,
        error_requests: Vec<ErrorRefreshRequest>,
    }

    impl DeviceDisplay for FakeDisplay {
        type Error = FakeError;

        fn refresh(&mut self, request: DisplayRefreshRequest) -> Result<(), Self::Error> {
            self.requests.push(request);
            self.result.take().unwrap_or(Ok(()))
        }

        fn refresh_error_page(&mut self, request: ErrorRefreshRequest) -> Result<(), Self::Error> {
            self.error_requests.push(request);
            self.result.take().unwrap_or(Ok(()))
        }

        fn has_image(&self, sha256: &str) -> bool {
            self.images.iter().any(|image| image == sha256)
        }
    }

    fn date(value: &str) -> LocalDate {
        LocalDate::parse(value).unwrap()
    }

    fn config() -> Config {
        Config {
            wifi_ssid: "wifi".to_string(),
            wifi_password: "password".to_string(),
            base_url: "https://example.com".to_string(),
            secret_key: "secret".to_string(),
        }
    }

    fn plan(image: &str) -> Plan {
        Plan {
            date: date("2026-06-08"),
            caption: "caption".to_string(),
            image: image.to_string(),
        }
    }

    fn input(plans: Option<Vec<Plan>>) -> DeviceCycleInput {
        DeviceCycleInput {
            config: Some(config()),
            plans,
            persistent_state: PersistentDeviceState::default(),
            trigger: RunTrigger::Wake(WakeReason::Timer),
            now_epoch_seconds: 100,
            date: date("2026-06-08"),
            battery: BatteryStatus::unknown(),
        }
    }

    #[test]
    fn sync_updates_plans_and_refreshes_display() {
        let remote_plans = vec![plan("a")];
        let mut sync = FakeSync {
            result: Some(Ok(SyncResult {
                plans: remote_plans.clone(),
                sprites: SpriteSet::default(),
                sprites_changed: false,
            })),
            requests: Vec::new(),
        };
        let mut display = FakeDisplay {
            images: vec!["a".to_string()],
            ..FakeDisplay::default()
        };

        let result = run_device_cycle(input(None), &mut sync, &mut display);

        assert_eq!(sync.requests.len(), 1);
        assert_eq!(display.requests.len(), 1);
        assert_eq!(display.requests[0].plan.image, "a");
        assert!(result.sync_succeeded);
        assert!(result.refresh_succeeded);
        assert_eq!(result.persistent_state.image.as_deref(), Some("a"));
    }

    #[test]
    fn low_battery_skips_sync_but_refreshes_current_state() {
        let plans = vec![plan("a")];
        let mut input = input(Some(plans));
        input.persistent_state = PersistentDeviceState::from_plan(&plan("a"), None);
        input.battery.low_battery = true;
        let mut sync = FakeSync::default();
        let mut display = FakeDisplay {
            images: vec!["a".to_string()],
            ..FakeDisplay::default()
        };

        let result = run_device_cycle(input, &mut sync, &mut display);

        assert!(sync.requests.is_empty());
        assert_eq!(display.requests.len(), 1);
        assert_eq!(display.requests[0].notice, Some(RenderNotice::LowBattery));
        assert_eq!(result.outcome, DeviceCycleOutcome::LowBatterySkipSync);
        assert_eq!(
            result.persistent_state.notice,
            Some(RenderNotice::LowBattery)
        );
    }

    #[test]
    fn low_battery_does_not_refresh_when_notice_is_already_displayed() {
        let plans = vec![plan("a")];
        let mut input = input(Some(plans));
        input.battery.low_battery = true;
        input.persistent_state =
            PersistentDeviceState::from_plan(&plan("a"), Some(RenderNotice::LowBattery));
        let mut sync = FakeSync::default();
        let mut display = FakeDisplay {
            images: vec!["a".to_string()],
            ..FakeDisplay::default()
        };

        let result = run_device_cycle(input, &mut sync, &mut display);

        assert!(sync.requests.is_empty());
        assert!(display.requests.is_empty());
        assert_eq!(result.outcome, DeviceCycleOutcome::LowBatterySkipSync);
    }

    #[test]
    fn charging_low_battery_status_still_syncs() {
        let mut input = input(Some(vec![plan("a")]));
        input.battery = BatteryStatus::new(0, Some(0), crate::power::ChargeState::Charging, true);
        let mut sync = FakeSync {
            result: Some(Ok(SyncResult {
                plans: vec![plan("a")],
                sprites: SpriteSet::default(),
                sprites_changed: false,
            })),
            requests: Vec::new(),
        };
        let mut display = FakeDisplay {
            images: vec!["a".to_string()],
            ..FakeDisplay::default()
        };

        let result = run_device_cycle(input, &mut sync, &mut display);

        assert_eq!(sync.requests.len(), 1);
        assert!(result.sync_attempted);
        assert!(result.sync_succeeded);
        assert!(sync.requests[0].notice.is_none());
        assert_ne!(result.outcome, DeviceCycleOutcome::LowBatterySkipSync);
    }

    #[test]
    fn charging_clears_previous_low_battery_notice_after_sync() {
        let mut input = input(Some(vec![plan("a")]));
        input.persistent_state =
            PersistentDeviceState::from_plan(&plan("a"), Some(RenderNotice::LowBattery));
        input.battery = BatteryStatus::new(0, Some(100), crate::power::ChargeState::Full, false);
        let mut sync = FakeSync {
            result: Some(Ok(SyncResult {
                plans: vec![plan("a")],
                sprites: SpriteSet::default(),
                sprites_changed: false,
            })),
            requests: Vec::new(),
        };
        let mut display = FakeDisplay {
            images: vec!["a".to_string()],
            ..FakeDisplay::default()
        };

        let result = run_device_cycle(input, &mut sync, &mut display);

        assert_eq!(sync.requests.len(), 1);
        assert_eq!(display.requests.len(), 1);
        assert_eq!(display.requests[0].notice, None);
        assert_eq!(result.persistent_state.notice, None);
    }

    #[test]
    fn sync_failure_without_displayable_cache_refreshes_error_page() {
        let mut sync = FakeSync {
            result: Some(Err(FakeError("network down"))),
            requests: Vec::new(),
        };
        let mut display = FakeDisplay::default();

        let result = run_device_cycle(input(Some(vec![plan("a")])), &mut sync, &mut display);

        assert_eq!(sync.requests.len(), 1);
        assert!(display.requests.is_empty());
        assert_eq!(display.error_requests.len(), 1);
        assert_eq!(display.error_requests[0].title, "SYNC ERROR");
        assert_eq!(
            result.outcome,
            DeviceCycleOutcome::NoUsablePhoto(NoUsablePhotoReason::ResourceNotCached)
        );
    }
}
