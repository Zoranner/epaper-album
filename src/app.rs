use crate::power::BatteryStatus;
use crate::schedule::{display_needs_refresh, select_plan_for_date};
use crate::state::{PersistentDeviceState, WakeReason};
use crate::{
    model::{DisplayItem, DisplayState, LocalDate, PlanSnapshot, ResourceIndex},
    state::RefreshReason,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunTrigger {
    Startup,
    Wake(WakeReason),
    Manual,
}

impl RunTrigger {
    pub const fn wake_reason(self) -> WakeReason {
        match self {
            Self::Startup => WakeReason::Startup,
            Self::Wake(reason) => reason,
            Self::Manual => WakeReason::ManualButton,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunOutcome {
    SyncRequested,
    RefreshOnly,
    SleepOnly,
    LowBatterySkipSync,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunInput {
    pub trigger: RunTrigger,
    pub now_epoch_seconds: u64,
    pub daily_sync_due: bool,
    pub display_refresh_due: bool,
    pub battery: BatteryStatus,
    pub persistent_state: PersistentDeviceState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunReport {
    pub trigger: RunTrigger,
    pub outcome: RunOutcome,
    pub wake_reason: WakeReason,
    pub force_sync: bool,
    pub daily_sync_consumed: bool,
    pub display_refresh_due: bool,
    pub battery: BatteryStatus,
    pub persistent_state: PersistentDeviceState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DisplayDecision {
    MissingConfig,
    NoUsablePhoto(NoUsablePhotoReason),
    RefreshRequired {
        item: DisplayItem,
        display_state: DisplayState,
        reason: RefreshReason,
    },
    SleepOnly {
        display_state: DisplayState,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NoUsablePhotoReason {
    NoPlanForDate,
    PlanHasNoImages,
    ResourceNotCached,
}

pub fn generate_display_decision(
    snapshot: Option<&PlanSnapshot>,
    resource_index: &ResourceIndex,
    date: LocalDate,
    rotation_slot: u64,
    previous_state: Option<&PersistentDeviceState>,
) -> DisplayDecision {
    let Some(snapshot) = snapshot else {
        return DisplayDecision::MissingConfig;
    };

    generate_display_decision_from_snapshot(
        snapshot,
        resource_index,
        date,
        rotation_slot,
        previous_state,
    )
}

pub fn generate_display_decision_from_snapshot(
    snapshot: &PlanSnapshot,
    resource_index: &ResourceIndex,
    date: LocalDate,
    rotation_slot: u64,
    previous_state: Option<&PersistentDeviceState>,
) -> DisplayDecision {
    let Some(item) = select_usable_display_item(snapshot, resource_index, date, rotation_slot)
    else {
        let reason = no_usable_photo_reason(snapshot, resource_index, date);
        return DisplayDecision::NoUsablePhoto(reason);
    };

    let previous_display = previous_state.map(PersistentDeviceState::display_state);
    let display_state = DisplayState::from(&item);

    if display_needs_refresh(previous_display.as_ref(), &item) {
        let reason = if previous_display
            .as_ref()
            .and_then(|state| state.image_sha256.as_deref())
            .is_some()
        {
            RefreshReason::DisplayItemChanged
        } else {
            RefreshReason::FirstBoot
        };

        return DisplayDecision::RefreshRequired {
            item,
            display_state,
            reason,
        };
    }

    DisplayDecision::SleepOnly { display_state }
}

fn select_usable_display_item(
    snapshot: &PlanSnapshot,
    resource_index: &ResourceIndex,
    date: LocalDate,
    rotation_slot: u64,
) -> Option<DisplayItem> {
    let plan = select_plan_for_date(&snapshot.plans, date)?;

    if plan.images.is_empty() {
        return None;
    }

    let image_count = plan.images.len();
    let start_index = (rotation_slot % image_count as u64) as usize;

    (0..image_count)
        .map(|offset| (start_index + offset) % image_count)
        .find_map(|image_index| {
            let image_sha256 = plan.images.get(image_index)?;
            resource_index.contains(image_sha256).then(|| DisplayItem {
                plan_id: plan.id,
                plan_content_hash: Some(snapshot.content_hash.clone()),
                date,
                image_sha256: image_sha256.clone(),
                image_index,
                caption: plan.caption.clone(),
            })
        })
}

fn no_usable_photo_reason(
    snapshot: &PlanSnapshot,
    resource_index: &ResourceIndex,
    date: LocalDate,
) -> NoUsablePhotoReason {
    let Some(plan) = select_plan_for_date(&snapshot.plans, date) else {
        return NoUsablePhotoReason::NoPlanForDate;
    };

    if plan.images.is_empty() {
        return NoUsablePhotoReason::PlanHasNoImages;
    }

    if plan
        .images
        .iter()
        .all(|image_sha256| !resource_index.contains(image_sha256))
    {
        return NoUsablePhotoReason::ResourceNotCached;
    }

    NoUsablePhotoReason::ResourceNotCached
}

pub fn run_once(mut input: RunInput) -> RunReport {
    let wake_reason = input.trigger.wake_reason();
    input.persistent_state.last_wake_reason = Some(wake_reason);

    let force_sync = matches!(input.trigger, RunTrigger::Manual);
    let sync_requested = force_sync || input.daily_sync_due;
    let daily_sync_consumed = input.daily_sync_due && !force_sync && !input.battery.low_battery;

    let outcome = if sync_requested && input.battery.low_battery {
        RunOutcome::LowBatterySkipSync
    } else if sync_requested {
        RunOutcome::SyncRequested
    } else if input.display_refresh_due {
        RunOutcome::RefreshOnly
    } else {
        RunOutcome::SleepOnly
    };

    RunReport {
        trigger: input.trigger,
        outcome,
        wake_reason,
        force_sync,
        daily_sync_consumed,
        display_refresh_due: input.display_refresh_due,
        battery: input.battery,
        persistent_state: input.persistent_state,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CachedResource, PlanItem};

    fn input(trigger: RunTrigger) -> RunInput {
        RunInput {
            trigger,
            now_epoch_seconds: 1,
            daily_sync_due: false,
            display_refresh_due: false,
            battery: BatteryStatus::unknown(),
            persistent_state: PersistentDeviceState::default(),
        }
    }

    #[test]
    fn manual_run_forces_sync_without_consuming_daily_sync() {
        let mut input = input(RunTrigger::Manual);
        input.daily_sync_due = true;

        let report = run_once(input);

        assert_eq!(report.outcome, RunOutcome::SyncRequested);
        assert!(report.force_sync);
        assert!(!report.daily_sync_consumed);
        assert_eq!(report.wake_reason, WakeReason::ManualButton);
        assert_eq!(
            report.persistent_state.last_wake_reason,
            Some(WakeReason::ManualButton)
        );
    }

    #[test]
    fn timer_wake_consumes_due_daily_sync() {
        let mut input = input(RunTrigger::Wake(WakeReason::Timer));
        input.daily_sync_due = true;

        let report = run_once(input);

        assert_eq!(report.outcome, RunOutcome::SyncRequested);
        assert!(!report.force_sync);
        assert!(report.daily_sync_consumed);
    }

    #[test]
    fn low_battery_skips_sync_without_consuming_daily_sync() {
        let mut input = input(RunTrigger::Wake(WakeReason::Timer));
        input.daily_sync_due = true;
        input.battery.low_battery = true;

        let report = run_once(input);

        assert_eq!(report.outcome, RunOutcome::LowBatterySkipSync);
        assert!(!report.daily_sync_consumed);
    }

    fn date(value: &str) -> LocalDate {
        LocalDate::parse(value).unwrap()
    }

    fn snapshot(images: &[&str]) -> PlanSnapshot {
        PlanSnapshot {
            content_hash: "hash-v1".to_string(),
            plans: vec![PlanItem {
                id: 7,
                start: date("2026-06-08"),
                end: date("2026-06-08"),
                caption: "caption".to_string(),
                images: images.iter().map(|image| image.to_string()).collect(),
            }],
        }
    }

    fn index(images: &[&str]) -> ResourceIndex {
        ResourceIndex {
            resources: images
                .iter()
                .map(|image| CachedResource {
                    sha256: image.to_string(),
                    byte_size: 128,
                    last_used_at_unix_secs: 1,
                })
                .collect(),
        }
    }

    #[test]
    fn display_decision_reports_missing_config() {
        let decision =
            generate_display_decision(None, &ResourceIndex::default(), date("2026-06-08"), 0, None);

        assert_eq!(decision, DisplayDecision::MissingConfig);
    }

    #[test]
    fn display_decision_reports_no_usable_photo_when_resource_is_missing() {
        let snapshot = snapshot(&["a"]);

        let decision = generate_display_decision_from_snapshot(
            &snapshot,
            &ResourceIndex::default(),
            date("2026-06-08"),
            0,
            None,
        );

        assert_eq!(
            decision,
            DisplayDecision::NoUsablePhoto(NoUsablePhotoReason::ResourceNotCached)
        );
    }

    #[test]
    fn display_decision_refreshes_first_usable_photo_from_rotation_slot() {
        let snapshot = snapshot(&["a", "b", "c"]);
        let index = index(&["c"]);

        let decision =
            generate_display_decision_from_snapshot(&snapshot, &index, date("2026-06-08"), 1, None);

        match decision {
            DisplayDecision::RefreshRequired {
                item,
                display_state,
                reason,
            } => {
                assert_eq!(item.image_sha256, "c");
                assert_eq!(item.image_index, 2);
                assert_eq!(display_state.image_sha256.as_deref(), Some("c"));
                assert_eq!(reason, RefreshReason::FirstBoot);
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn display_decision_sleeps_when_previous_state_matches() {
        let snapshot = snapshot(&["a"]);
        let index = index(&["a"]);
        let previous_display = DisplayState {
            plan_id: Some(7),
            plan_content_hash: Some("older-hash".to_string()),
            date: Some(date("2026-06-08")),
            image_sha256: Some("a".to_string()),
            image_index: 0,
            caption: Some("caption".to_string()),
            refreshed_at_unix_secs: Some(100),
        };
        let mut previous_state = PersistentDeviceState::default();
        previous_state.set_current_display(&previous_display);

        let decision = generate_display_decision_from_snapshot(
            &snapshot,
            &index,
            date("2026-06-08"),
            0,
            Some(&previous_state),
        );

        match decision {
            DisplayDecision::SleepOnly { display_state } => {
                assert_eq!(display_state.image_sha256.as_deref(), Some("a"));
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }
}
