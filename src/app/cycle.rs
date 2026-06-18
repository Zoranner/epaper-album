use crate::app::RunTrigger;
use crate::config::Config;
use crate::model::{LocalDate, Plan};
use crate::power::{BatteryStatus, PowerProfile};
use crate::state::{PersistentDeviceState, PersistentSyncState, RefreshReason};
use std::fmt;

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
pub struct SyncRequest {
    pub config: Config,
    pub local_plans: Option<Vec<Plan>>,
    pub date: LocalDate,
    pub now_epoch_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncResult {
    pub plans: Vec<Plan>,
    pub sprites: SpriteSet,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SpriteSet {
    pub caption: Option<String>,
    pub date: Option<String>,
}

pub trait DeviceCloudSync {
    type Error: fmt::Display;

    fn sync_resources(&mut self, request: SyncRequest) -> Result<SyncResult, Self::Error>;

    fn describe_error(&self, error: &Self::Error) -> SyncErrorReport {
        SyncErrorReport::from_display(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisplayRefreshRequest {
    pub plan: Plan,
    pub date: LocalDate,
    pub reason: RefreshReason,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncErrorReport {
    pub code: String,
    pub category: String,
    pub stage: Option<String>,
    pub message: String,
    pub detail: String,
}

impl SyncErrorReport {
    pub fn new(
        code: impl Into<String>,
        category: impl Into<String>,
        stage: Option<String>,
        message: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            category: category.into(),
            stage,
            message: message.into(),
            detail: detail.into(),
        }
    }

    pub fn from_display(error: &impl fmt::Display) -> Self {
        Self::new(
            "sync.error",
            "sync",
            None,
            "CANNOT UPDATE SERVER DATA",
            error.to_string(),
        )
    }
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
    NoUsablePhoto,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncAction {
    Fetch,
    Skip,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncCause {
    External,
    Daily,
    Done,
    LowBattery,
    MissingConfig,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyncDecision {
    pub action: SyncAction,
    pub cause: SyncCause,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DisplayAction {
    Keep,
    Refresh(DisplayTarget),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DisplayTarget {
    Photo {
        date: LocalDate,
        image: String,
        caption: String,
    },
    Page {
        date: LocalDate,
        title: String,
        message: String,
        hint: String,
        detail: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayCause {
    First,
    Date,
    Photo,
    LowBattery,
    Sync,
    MissingConfig,
    MissingPhoto,
    Same,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisplayDecision {
    pub action: DisplayAction,
    pub cause: DisplayCause,
}

pub fn decide_sync(
    config: Option<&Config>,
    battery: &BatteryStatus,
    sync_state: &PersistentSyncState,
    date: LocalDate,
) -> SyncDecision {
    if battery.effective_low_battery() {
        return SyncDecision {
            action: SyncAction::Skip,
            cause: SyncCause::LowBattery,
        };
    }

    if config.is_none_or(|config| !config.has_required_values()) {
        return SyncDecision {
            action: SyncAction::Skip,
            cause: SyncCause::MissingConfig,
        };
    }

    if matches!(PowerProfile::from(battery), PowerProfile::External) {
        return SyncDecision {
            action: SyncAction::Fetch,
            cause: SyncCause::External,
        };
    }

    if sync_state.date != Some(date) {
        return SyncDecision {
            action: SyncAction::Fetch,
            cause: SyncCause::Daily,
        };
    }

    SyncDecision {
        action: SyncAction::Skip,
        cause: SyncCause::Done,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunContext {
    pub now: u64,
    pub date: LocalDate,
    pub trigger: RunTrigger,
    pub battery: BatteryStatus,
    pub power: PowerProfile,
    pub config: Option<Config>,
    pub plans: Option<Vec<Plan>>,
    pub state: PersistentDeviceState,
    pub sync: PersistentSyncState,
}

pub fn decide_display(
    context: &RunContext,
    image_exists: impl Fn(&str) -> bool,
    sync_error: Option<&SyncErrorReport>,
) -> DisplayDecision {
    if context.config.is_none() {
        return DisplayDecision {
            action: DisplayAction::Refresh(DisplayTarget::Page {
                date: context.date,
                title: "CONFIG ERROR".to_string(),
                message: "DEVICE CONFIG IS MISSING".to_string(),
                hint: "CHECK /SDCARD/CONFIG.TOML".to_string(),
                detail: "WIFI BASE URL AND SECRET KEY REQUIRED".to_string(),
            }),
            cause: DisplayCause::MissingConfig,
        };
    }

    let effective_low_battery = context.battery.effective_low_battery();
    if effective_low_battery {
        return DisplayDecision {
            action: DisplayAction::Refresh(DisplayTarget::Page {
                date: context.date,
                title: "LOW BATTERY".to_string(),
                message: "BATTERY IS LOW".to_string(),
                hint: "CONNECT POWER".to_string(),
                detail: "CLOUD SYNC PAUSED".to_string(),
            }),
            cause: DisplayCause::LowBattery,
        };
    }

    let selected = context
        .plans
        .as_deref()
        .and_then(|plans| crate::schedule::select_plan_for_date(plans, context.date));

    if let Some(sync_error) = sync_error {
        return sync_error_report_decision(context.date, sync_error);
    }

    if let Some(plan) = selected {
        if !image_exists(&plan.image) {
            return missing_photo_decision(context.date);
        }

        if context.state.matches_display(context.date, plan) {
            return DisplayDecision {
                action: DisplayAction::Keep,
                cause: DisplayCause::Same,
            };
        }

        let cause = if context.state.image.is_none() {
            DisplayCause::First
        } else if context.state.date != Some(context.date) {
            DisplayCause::Date
        } else {
            DisplayCause::Photo
        };

        return DisplayDecision {
            action: DisplayAction::Refresh(photo_target_from_plan(context.date, plan)),
            cause,
        };
    }

    missing_photo_decision(context.date)
}

fn sync_error_report_decision(date: LocalDate, sync_error: &SyncErrorReport) -> DisplayDecision {
    DisplayDecision {
        action: DisplayAction::Refresh(DisplayTarget::Page {
            date,
            title: "SYNC ERROR".to_string(),
            message: sync_error.message.to_ascii_uppercase(),
            hint: "CHECK WIFI BASE URL AND SERVER".to_string(),
            detail: sync_error.detail.clone(),
        }),
        cause: DisplayCause::Sync,
    }
}

fn photo_target_from_plan(date: LocalDate, plan: &Plan) -> DisplayTarget {
    DisplayTarget::Photo {
        date,
        image: plan.image.clone(),
        caption: plan.caption.clone(),
    }
}

fn missing_photo_decision(date: LocalDate) -> DisplayDecision {
    DisplayDecision {
        action: DisplayAction::Refresh(DisplayTarget::Page {
            date,
            title: "NO PHOTO".to_string(),
            message: "NO DISPLAYABLE PHOTO".to_string(),
            hint: "CHECK SERVER PLAN AND IMAGE CACHE".to_string(),
            detail: "PHOTO RESOURCE IS MISSING".to_string(),
        }),
        cause: DisplayCause::MissingPhoto,
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
            let plan = Plan {
                date,
                image,
                caption,
            };
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
            persistent_state_loaded: false,
            sync_state: crate::state::PersistentSyncState::default(),
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
            })),
            requests: Vec::new(),
        };
        let mut display = FakeDisplay {
            images: vec!["a".to_string()],
            ..FakeDisplay::default()
        };

        let result = run_device_cycle(input(None), &mut sync, &mut display);

        assert_eq!(sync.requests.len(), 1);
        assert_eq!(result.sync_decision.action, SyncAction::Fetch);
        assert_eq!(result.sync_decision.cause, SyncCause::Daily);
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
        input.persistent_state = PersistentDeviceState::from_display(input.date, &plan("a"));
        input.battery.low_battery = true;
        let mut sync = FakeSync::default();
        let mut display = FakeDisplay {
            images: vec!["a".to_string()],
            ..FakeDisplay::default()
        };

        let result = run_device_cycle(input, &mut sync, &mut display);

        assert!(sync.requests.is_empty());
        assert_eq!(result.sync_decision.action, SyncAction::Skip);
        assert_eq!(result.sync_decision.cause, SyncCause::LowBattery);
        assert!(display.requests.is_empty());
        assert_eq!(display.error_requests.len(), 1);
        assert_eq!(display.error_requests[0].title, "LOW BATTERY");
        assert_eq!(result.outcome, DeviceCycleOutcome::LowBatterySkipSync);
        assert_eq!(result.persistent_state.image, None);
    }

    #[test]
    fn low_battery_refreshes_error_page_when_photo_state_matches() {
        let plans = vec![plan("a")];
        let mut input = input(Some(plans));
        input.battery.low_battery = true;
        input.persistent_state = PersistentDeviceState::from_display(input.date, &plan("a"));
        let mut sync = FakeSync::default();
        let mut display = FakeDisplay {
            images: vec!["a".to_string()],
            ..FakeDisplay::default()
        };

        let result = run_device_cycle(input, &mut sync, &mut display);

        assert!(sync.requests.is_empty());
        assert_eq!(result.sync_decision.action, SyncAction::Skip);
        assert_eq!(result.sync_decision.cause, SyncCause::LowBattery);
        assert!(display.requests.is_empty());
        assert_eq!(display.error_requests.len(), 1);
        assert_eq!(display.error_requests[0].title, "LOW BATTERY");
        assert_eq!(result.outcome, DeviceCycleOutcome::LowBatterySkipSync);
        assert!(result.display_available);
    }

    #[test]
    fn keep_decision_marks_display_available_when_state_was_loaded() {
        let plans = vec![plan("a")];
        let mut input = input(Some(plans));
        input.persistent_state_loaded = true;
        input.sync_state.date = Some(input.date);
        input.persistent_state = PersistentDeviceState::from_display(input.date, &plan("a"));
        let mut sync = FakeSync::default();
        let mut display = FakeDisplay {
            images: vec!["a".to_string()],
            ..FakeDisplay::default()
        };

        let result = run_device_cycle(input, &mut sync, &mut display);

        assert!(sync.requests.is_empty());
        assert!(display.requests.is_empty());
        assert!(result.display_available);
    }

    #[test]
    fn charging_low_battery_status_still_syncs() {
        let mut input = input(Some(vec![plan("a")]));
        input.battery = BatteryStatus::new(0, Some(0), crate::power::ChargeState::Charging, true);
        let mut sync = FakeSync {
            result: Some(Ok(SyncResult {
                plans: vec![plan("a")],
                sprites: SpriteSet::default(),
            })),
            requests: Vec::new(),
        };
        let mut display = FakeDisplay {
            images: vec!["a".to_string()],
            ..FakeDisplay::default()
        };

        let result = run_device_cycle(input, &mut sync, &mut display);

        assert_eq!(sync.requests.len(), 1);
        assert_eq!(result.sync_decision.action, SyncAction::Fetch);
        assert_eq!(result.sync_decision.cause, SyncCause::External);
        assert!(result.sync_attempted);
        assert!(result.sync_succeeded);
        assert_ne!(result.outcome, DeviceCycleOutcome::LowBatterySkipSync);
    }

    #[test]
    fn charging_keeps_screen_when_photo_is_unchanged_after_sync() {
        let mut input = input(Some(vec![plan("a")]));
        input.persistent_state_loaded = true;
        input.persistent_state = PersistentDeviceState::from_display(input.date, &plan("a"));
        input.battery = BatteryStatus::new(0, Some(100), crate::power::ChargeState::Full, false);
        let mut sync = FakeSync {
            result: Some(Ok(SyncResult {
                plans: vec![plan("a")],
                sprites: SpriteSet::default(),
            })),
            requests: Vec::new(),
        };
        let mut display = FakeDisplay {
            images: vec!["a".to_string()],
            ..FakeDisplay::default()
        };

        let result = run_device_cycle(input, &mut sync, &mut display);

        assert_eq!(sync.requests.len(), 1);
        assert_eq!(result.sync_decision.action, SyncAction::Fetch);
        assert_eq!(result.sync_decision.cause, SyncCause::External);
        assert!(display.requests.is_empty());
        assert!(display.error_requests.is_empty());
        assert_eq!(result.display_decision.cause, DisplayCause::Same);
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
        assert_eq!(display.error_requests[0].detail, "network down");
        assert_eq!(
            result.sync_error_report,
            Some(SyncErrorReport::new(
                "sync.error",
                "sync",
                None,
                "CANNOT UPDATE SERVER DATA",
                "network down"
            ))
        );
        assert_eq!(result.outcome, DeviceCycleOutcome::SyncFailed);
    }

    #[test]
    fn sync_failure_refreshes_error_page_even_when_photo_is_usable() {
        let mut input = input(Some(vec![plan("a")]));
        input.persistent_state_loaded = true;
        input.persistent_state = PersistentDeviceState::from_display(input.date, &plan("a"));
        input.sync_state.date = Some(date("2026-06-07"));
        let mut sync = FakeSync {
            result: Some(Err(FakeError("network down"))),
            requests: Vec::new(),
        };
        let mut display = FakeDisplay {
            images: vec!["a".to_string()],
            ..FakeDisplay::default()
        };

        let result = run_device_cycle(input, &mut sync, &mut display);

        assert_eq!(sync.requests.len(), 1);
        assert!(display.requests.is_empty());
        assert_eq!(display.error_requests.len(), 1);
        assert_eq!(display.error_requests[0].title, "SYNC ERROR");
        assert_eq!(display.error_requests[0].detail, "network down");
        assert_eq!(result.display_decision.cause, DisplayCause::Sync);
        assert_eq!(result.persistent_state.image, None);
        assert_eq!(result.outcome, DeviceCycleOutcome::SyncFailed);
    }

    #[test]
    fn missing_today_plan_keeps_latest_past_plan_without_sync_error() {
        let plans = vec![Plan {
            date: date("2026-06-16"),
            caption: "past".to_string(),
            image: "a".to_string(),
        }];
        let mut input = input(Some(plans.clone()));
        input.date = date("2026-06-18");
        input.sync_state.date = Some(date("2026-06-18"));
        let mut sync = FakeSync::default();
        let mut display = FakeDisplay {
            images: vec!["a".to_string()],
            ..FakeDisplay::default()
        };

        let result = run_device_cycle(input, &mut sync, &mut display);

        assert!(sync.requests.is_empty());
        assert_eq!(display.requests.len(), 1);
        assert!(display.error_requests.is_empty());
        assert_eq!(display.requests[0].date, date("2026-06-18"));
        assert_eq!(display.requests[0].plan.image, "a");
        assert_eq!(result.display_decision.cause, DisplayCause::First);
        assert_ne!(result.outcome, DeviceCycleOutcome::SyncFailed);
    }

    #[test]
    fn sync_success_keeps_screen_when_photo_is_unchanged() {
        let mut input = input(Some(vec![plan("a")]));
        input.persistent_state_loaded = true;
        input.persistent_state = PersistentDeviceState::from_display(input.date, &plan("a"));
        input.sync_state.date = Some(date("2026-06-07"));
        let mut sync = FakeSync {
            result: Some(Ok(SyncResult {
                plans: vec![plan("a")],
                sprites: SpriteSet::default(),
            })),
            requests: Vec::new(),
        };
        let mut display = FakeDisplay {
            images: vec!["a".to_string()],
            ..FakeDisplay::default()
        };

        let result = run_device_cycle(input, &mut sync, &mut display);

        assert_eq!(sync.requests.len(), 1);
        assert!(result.sync_succeeded);
        assert!(display.requests.is_empty());
        assert!(display.error_requests.is_empty());
        assert_eq!(result.display_decision.cause, DisplayCause::Same);
        assert_eq!(result.outcome, DeviceCycleOutcome::SyncRequested);
    }

    #[test]
    fn battery_skips_sync_when_plan_already_synced_today() {
        let mut input = input(Some(vec![plan("a")]));
        input.sync_state.date = Some(date("2026-06-08"));
        input.persistent_state =
            PersistentDeviceState::from_display(date("2026-06-08"), &plan("a"));
        let mut sync = FakeSync::default();
        let mut display = FakeDisplay {
            images: vec!["a".to_string()],
            ..FakeDisplay::default()
        };

        let result = run_device_cycle(input, &mut sync, &mut display);

        assert!(sync.requests.is_empty());
        assert_eq!(result.sync_decision.action, SyncAction::Skip);
        assert_eq!(result.sync_decision.cause, SyncCause::Done);
        assert!(!result.sync_attempted);
    }

    #[test]
    fn battery_requests_sync_when_plan_not_synced_today() {
        let mut input = input(Some(vec![plan("a")]));
        input.sync_state.date = Some(date("2026-06-07"));
        let mut sync = FakeSync {
            result: Some(Ok(SyncResult {
                plans: vec![plan("a")],
                sprites: SpriteSet::default(),
            })),
            requests: Vec::new(),
        };
        let mut display = FakeDisplay {
            images: vec!["a".to_string()],
            ..FakeDisplay::default()
        };

        let result = run_device_cycle(input, &mut sync, &mut display);

        assert_eq!(sync.requests.len(), 1);
        assert_eq!(result.sync_decision.action, SyncAction::Fetch);
        assert_eq!(result.sync_decision.cause, SyncCause::Daily);
        assert!(result.sync_attempted);
        assert!(result.sync_succeeded);
        assert_eq!(result.sync_state.date, Some(date("2026-06-08")));
    }

    #[test]
    fn external_power_syncs_but_keeps_screen_when_display_is_unchanged() {
        let plans = vec![plan("a")];
        let mut input = input(Some(plans.clone()));
        input.persistent_state_loaded = true;
        input.persistent_state = PersistentDeviceState::from_display(input.date, &plans[0]);
        input.battery = BatteryStatus::new(0, Some(100), crate::power::ChargeState::Full, false);
        let mut sync = FakeSync {
            result: Some(Ok(SyncResult {
                plans,
                sprites: SpriteSet::default(),
            })),
            requests: Vec::new(),
        };
        let mut display = FakeDisplay {
            images: vec!["a".to_string()],
            ..FakeDisplay::default()
        };

        let result = run_device_cycle(input, &mut sync, &mut display);

        assert_eq!(result.sync_decision.action, SyncAction::Fetch);
        assert_eq!(result.sync_decision.cause, SyncCause::External);
        assert!(result.sync_attempted);
        assert!(result.sync_succeeded);
        assert!(display.requests.is_empty());
        assert_eq!(result.display_decision.cause, DisplayCause::Same);
        assert_eq!(result.outcome, DeviceCycleOutcome::SyncRequested);
        assert!(result.display_available);
    }
}
