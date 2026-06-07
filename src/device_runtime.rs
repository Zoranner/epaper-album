use crate::app::{
    generate_display_decision, DisplayDecision, NoUsablePhotoReason, RunOutcome, RunTrigger,
};
use crate::cache::missing_resources;
use crate::config::Config;
use crate::model::{
    CachedResource, DisplayItem, DisplayState, LocalDate, PlanSnapshot, ResourceIndex,
};
use crate::power::BatteryStatus;
use crate::state::{PersistentDeviceState, RefreshReason};
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceCycleInput {
    pub config: Option<Config>,
    pub snapshot: Option<PlanSnapshot>,
    pub resource_index: ResourceIndex,
    pub persistent_state: PersistentDeviceState,
    pub trigger: RunTrigger,
    pub now_epoch_seconds: u64,
    pub date: LocalDate,
    pub rotation_slot: u64,
    pub battery: BatteryStatus,
    pub daily_sync_due: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncRequest {
    pub config: Config,
    pub local_snapshot: Option<PlanSnapshot>,
    pub resource_index: ResourceIndex,
    pub missing_resources: Vec<String>,
    pub force: bool,
    pub now_epoch_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncResult {
    pub snapshot: PlanSnapshot,
    pub resources: Vec<CachedResource>,
}

pub trait DeviceCloudSync {
    type Error: fmt::Display;

    fn sync_resources(&mut self, request: SyncRequest) -> Result<SyncResult, Self::Error>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisplayRefreshRequest {
    pub item: DisplayItem,
    pub display_state: DisplayState,
    pub reason: RefreshReason,
    pub now_epoch_seconds: u64,
}

pub trait DeviceDisplay {
    type Error: fmt::Display;

    fn refresh(&mut self, request: DisplayRefreshRequest) -> Result<(), Self::Error>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceCycleResult {
    pub snapshot: Option<PlanSnapshot>,
    pub resource_index: ResourceIndex,
    pub persistent_state: PersistentDeviceState,
    pub display_decision: DisplayDecision,
    pub outcome: DeviceCycleOutcome,
    pub sync_attempted: bool,
    pub sync_succeeded: bool,
    pub daily_sync_consumed: bool,
    pub force_sync: bool,
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
        mut snapshot,
        mut resource_index,
        mut persistent_state,
        trigger,
        now_epoch_seconds,
        date,
        rotation_slot,
        battery,
        daily_sync_due,
    } = input;

    let wake_reason = trigger.wake_reason();
    persistent_state.last_wake_reason = Some(wake_reason);

    let force_sync = matches!(trigger, RunTrigger::Manual);
    let sync_requested = force_sync || daily_sync_due;
    let mut sync_attempted = false;
    let mut sync_succeeded = false;
    let mut daily_sync_consumed = false;
    let mut sync_failed = false;

    if sync_requested && !battery.low_battery {
        if let Some(config) = config
            .as_ref()
            .filter(|config| config.has_required_values())
        {
            sync_attempted = true;
            let missing = snapshot
                .as_ref()
                .map(|snapshot| missing_resources(snapshot, &resource_index))
                .unwrap_or_default();
            let request = SyncRequest {
                config: config.clone(),
                local_snapshot: snapshot.clone(),
                resource_index: resource_index.clone(),
                missing_resources: missing,
                force: force_sync,
                now_epoch_seconds,
            };

            match sync.sync_resources(request) {
                Ok(sync_result) => {
                    snapshot = Some(sync_result.snapshot);
                    for mut resource in sync_result.resources {
                        resource.last_used_at_unix_secs = now_epoch_seconds;
                        resource_index.upsert(resource);
                    }
                    persistent_state.last_successful_sync_epoch_seconds = Some(now_epoch_seconds);
                    persistent_state.last_sync_error = None;
                    sync_succeeded = true;
                    daily_sync_consumed = daily_sync_due && !force_sync;
                }
                Err(error) => {
                    persistent_state.last_sync_error = Some(error.to_string());
                    sync_failed = true;
                }
            }
        } else {
            persistent_state.last_sync_error = Some("missing or incomplete config".to_string());
        }
    }

    update_cache_state(&mut persistent_state, &resource_index);

    let decision = generate_display_decision(
        snapshot.as_ref(),
        &resource_index,
        date,
        rotation_slot,
        Some(&persistent_state),
    );

    let mut refresh_attempted = false;
    let mut refresh_succeeded = false;
    let mut refresh_failed = false;

    if let DisplayDecision::RefreshRequired {
        item,
        display_state,
        reason,
    } = &decision
    {
        refresh_attempted = true;
        let mut next_display_state = display_state.clone();
        next_display_state.refreshed_at_unix_secs = Some(now_epoch_seconds);
        let request = DisplayRefreshRequest {
            item: item.clone(),
            display_state: next_display_state.clone(),
            reason: *reason,
            now_epoch_seconds,
        };

        match display.refresh(request) {
            Ok(()) => {
                persistent_state.set_current_display(&next_display_state);
                persistent_state.last_refresh_reason = Some(*reason);
                resource_index.touch(&item.image_sha256, now_epoch_seconds);
                update_cache_state(&mut persistent_state, &resource_index);
                refresh_succeeded = true;
            }
            Err(error) => {
                persistent_state.last_sync_error = Some(error.to_string());
                refresh_failed = true;
            }
        }
    }

    let outcome = cycle_outcome(
        &decision,
        config.as_ref(),
        sync_requested,
        battery.low_battery,
        sync_failed,
        refresh_failed,
        refresh_attempted,
    );

    DeviceCycleResult {
        snapshot,
        resource_index,
        persistent_state,
        display_decision: decision,
        outcome,
        sync_attempted,
        sync_succeeded,
        daily_sync_consumed,
        force_sync,
        refresh_attempted,
        refresh_succeeded,
    }
}

fn update_cache_state(state: &mut PersistentDeviceState, resource_index: &ResourceIndex) {
    state.cache.resource_count = resource_index.resources.len() as u32;
    state.cache.used_bytes = resource_index
        .resources
        .iter()
        .map(|resource| resource.byte_size)
        .sum();
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
    use crate::model::PlanItem;
    use crate::power::BatteryStatus;
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
        result: Option<Result<(), FakeError>>,
        requests: Vec<DisplayRefreshRequest>,
    }

    impl DeviceDisplay for FakeDisplay {
        type Error = FakeError;

        fn refresh(&mut self, request: DisplayRefreshRequest) -> Result<(), Self::Error> {
            self.requests.push(request);
            self.result.take().unwrap_or(Ok(()))
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

    fn snapshot(hash: &str, images: &[&str]) -> PlanSnapshot {
        PlanSnapshot {
            content_hash: hash.to_string(),
            plans: vec![PlanItem {
                id: 7,
                start: date("2026-06-08"),
                end: date("2026-06-08"),
                caption: "caption".to_string(),
                images: images.iter().map(|image| image.to_string()).collect(),
            }],
        }
    }

    fn resource(sha256: &str, byte_size: u64, last_used_at_unix_secs: u64) -> CachedResource {
        CachedResource {
            sha256: sha256.to_string(),
            byte_size,
            last_used_at_unix_secs,
        }
    }

    fn index(resources: &[CachedResource]) -> ResourceIndex {
        ResourceIndex {
            resources: resources.to_vec(),
        }
    }

    fn input(snapshot: Option<PlanSnapshot>, resource_index: ResourceIndex) -> DeviceCycleInput {
        DeviceCycleInput {
            config: Some(config()),
            snapshot,
            resource_index,
            persistent_state: PersistentDeviceState::default(),
            trigger: RunTrigger::Wake(WakeReason::Timer),
            now_epoch_seconds: 100,
            date: date("2026-06-08"),
            rotation_slot: 0,
            battery: BatteryStatus::unknown(),
            daily_sync_due: false,
        }
    }

    #[test]
    fn sync_downloads_missing_image_and_refreshes_display() {
        let local_snapshot = snapshot("hash-local", &["a"]);
        let remote_snapshot = snapshot("hash-remote", &["a"]);
        let mut input = input(Some(local_snapshot), ResourceIndex::default());
        input.daily_sync_due = true;
        let mut sync = FakeSync {
            result: Some(Ok(SyncResult {
                snapshot: remote_snapshot.clone(),
                resources: vec![resource("a", 128, 0)],
            })),
            requests: Vec::new(),
        };
        let mut display = FakeDisplay::default();

        let result = run_device_cycle(input, &mut sync, &mut display);

        assert_eq!(sync.requests.len(), 1);
        assert_eq!(sync.requests[0].missing_resources, vec!["a"]);
        assert_eq!(display.requests.len(), 1);
        assert_eq!(
            display.requests[0].display_state.image_sha256.as_deref(),
            Some("a")
        );
        assert!(result.sync_succeeded);
        assert!(result.daily_sync_consumed);
        assert!(result.refresh_succeeded);
        assert_eq!(
            result
                .persistent_state
                .current_display
                .image_sha256
                .as_deref(),
            Some("a")
        );
        assert_eq!(
            result.persistent_state.last_refresh_reason,
            Some(RefreshReason::FirstBoot)
        );
        assert_eq!(
            result.persistent_state.last_successful_sync_epoch_seconds,
            Some(100)
        );
        assert_eq!(result.persistent_state.cache.resource_count, 1);
        assert_eq!(result.persistent_state.cache.used_bytes, 128);
    }

    #[test]
    fn low_battery_skips_sync_but_refreshes_from_cache() {
        let mut input = input(
            Some(snapshot("hash-v1", &["a"])),
            index(&[resource("a", 128, 1)]),
        );
        input.daily_sync_due = true;
        input.battery.low_battery = true;
        let mut sync = FakeSync::default();
        let mut display = FakeDisplay::default();

        let result = run_device_cycle(input, &mut sync, &mut display);

        assert!(sync.requests.is_empty());
        assert_eq!(display.requests.len(), 1);
        assert_eq!(result.outcome, DeviceCycleOutcome::LowBatterySkipSync);
        assert!(!result.daily_sync_consumed);
        assert!(result.refresh_succeeded);
    }

    #[test]
    fn manual_force_sync_does_not_consume_daily_task() {
        let mut input = input(
            Some(snapshot("hash-v1", &["a"])),
            index(&[resource("a", 128, 1)]),
        );
        input.trigger = RunTrigger::Manual;
        input.daily_sync_due = true;
        let mut sync = FakeSync {
            result: Some(Ok(SyncResult {
                snapshot: snapshot("hash-v2", &["a"]),
                resources: Vec::new(),
            })),
            requests: Vec::new(),
        };
        let mut display = FakeDisplay::default();

        let result = run_device_cycle(input, &mut sync, &mut display);

        assert_eq!(sync.requests.len(), 1);
        assert!(sync.requests[0].force);
        assert!(result.force_sync);
        assert!(!result.daily_sync_consumed);
        assert_eq!(
            result.persistent_state.last_wake_reason,
            Some(WakeReason::ManualButton)
        );
    }

    #[test]
    fn sync_failure_uses_local_cache_for_display_decision() {
        let mut input = input(
            Some(snapshot("hash-v1", &["a"])),
            index(&[resource("a", 128, 1)]),
        );
        input.daily_sync_due = true;
        let mut sync = FakeSync {
            result: Some(Err(FakeError("network down"))),
            requests: Vec::new(),
        };
        let mut display = FakeDisplay::default();

        let result = run_device_cycle(input, &mut sync, &mut display);

        assert_eq!(sync.requests.len(), 1);
        assert_eq!(display.requests.len(), 1);
        assert_eq!(result.outcome, DeviceCycleOutcome::SyncFailed);
        assert_eq!(
            result.persistent_state.last_sync_error.as_deref(),
            Some("network down")
        );
        assert!(result.refresh_succeeded);
    }

    #[test]
    fn no_available_photo_returns_no_usable_photo_decision() {
        let input = input(Some(snapshot("hash-v1", &["a"])), ResourceIndex::default());
        let mut sync = FakeSync::default();
        let mut display = FakeDisplay::default();

        let result = run_device_cycle(input, &mut sync, &mut display);

        assert_eq!(sync.requests.len(), 0);
        assert_eq!(display.requests.len(), 0);
        assert_eq!(
            result.display_decision,
            DisplayDecision::NoUsablePhoto(NoUsablePhotoReason::ResourceNotCached)
        );
        assert_eq!(
            result.outcome,
            DeviceCycleOutcome::NoUsablePhoto(NoUsablePhotoReason::ResourceNotCached)
        );
    }
}
