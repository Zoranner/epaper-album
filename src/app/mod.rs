pub mod cycle;

use crate::power::BatteryStatus;
use crate::schedule::{display_needs_refresh, select_display_item, select_plan_for_date};
use crate::state::{PersistentDeviceState, WakeReason};
use crate::{
    model::{DisplayItem, DisplayState, LocalDate, PlanSnapshot, ResourceIndex},
    state::RefreshReason,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunTrigger {
    Startup,
    Wake(WakeReason),
}

impl RunTrigger {
    pub const fn wake_reason(self) -> WakeReason {
        match self {
            Self::Startup => WakeReason::Startup,
            Self::Wake(reason) => reason,
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
    ResourceNotCached,
}

pub fn generate_display_decision(
    snapshot: Option<&PlanSnapshot>,
    resource_index: &ResourceIndex,
    date: LocalDate,
    previous_state: Option<&PersistentDeviceState>,
) -> DisplayDecision {
    let Some(snapshot) = snapshot else {
        return DisplayDecision::MissingConfig;
    };

    generate_display_decision_from_snapshot(snapshot, resource_index, date, previous_state)
}

pub fn generate_display_decision_from_snapshot(
    snapshot: &PlanSnapshot,
    resource_index: &ResourceIndex,
    date: LocalDate,
    previous_state: Option<&PersistentDeviceState>,
) -> DisplayDecision {
    let Some(item) = select_usable_display_item(snapshot, resource_index, date) else {
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
) -> Option<DisplayItem> {
    let item = select_display_item(snapshot, date)?;

    resource_index.contains(&item.image_sha256).then_some(item)
}

fn no_usable_photo_reason(
    snapshot: &PlanSnapshot,
    resource_index: &ResourceIndex,
    date: LocalDate,
) -> NoUsablePhotoReason {
    let Some(plan) = select_plan_for_date(&snapshot.plans, date) else {
        return NoUsablePhotoReason::NoPlanForDate;
    };

    if !resource_index.contains(&plan.image_sha256) {
        return NoUsablePhotoReason::ResourceNotCached;
    }

    NoUsablePhotoReason::ResourceNotCached
}

pub fn run_once(mut input: RunInput) -> RunReport {
    let wake_reason = input.trigger.wake_reason();
    input.persistent_state.last_wake_reason = Some(wake_reason);

    let sync_requested = true;
    let daily_sync_consumed = input.daily_sync_due && !input.battery.low_battery;

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
    fn startup_run_requests_sync_and_consumes_due_daily_sync() {
        let mut input = input(RunTrigger::Startup);
        input.daily_sync_due = true;

        let report = run_once(input);

        assert_eq!(report.outcome, RunOutcome::SyncRequested);
        assert!(report.daily_sync_consumed);
        assert_eq!(report.wake_reason, WakeReason::Startup);
        assert_eq!(
            report.persistent_state.last_wake_reason,
            Some(WakeReason::Startup)
        );
    }

    #[test]
    fn timer_wake_consumes_due_daily_sync() {
        let mut input = input(RunTrigger::Wake(WakeReason::Timer));
        input.daily_sync_due = true;

        let report = run_once(input);

        assert_eq!(report.outcome, RunOutcome::SyncRequested);
        assert!(report.daily_sync_consumed);
    }

    #[test]
    fn timer_wake_requests_sync_even_when_daily_sync_is_not_due() {
        let input = input(RunTrigger::Wake(WakeReason::Timer));

        let report = run_once(input);

        assert_eq!(report.outcome, RunOutcome::SyncRequested);
        assert!(!report.daily_sync_consumed);
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

    fn snapshot(image_sha256: &str) -> PlanSnapshot {
        PlanSnapshot {
            content_hash: "hash-v1".to_string(),
            plans: vec![PlanItem {
                date: date("2026-06-08"),
                caption: "caption".to_string(),
                image_sha256: image_sha256.to_string(),
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
            generate_display_decision(None, &ResourceIndex::default(), date("2026-06-08"), None);

        assert_eq!(decision, DisplayDecision::MissingConfig);
    }

    #[test]
    fn display_decision_reports_no_usable_photo_when_resource_is_missing() {
        let snapshot = snapshot("a");

        let decision = generate_display_decision_from_snapshot(
            &snapshot,
            &ResourceIndex::default(),
            date("2026-06-08"),
            None,
        );

        assert_eq!(
            decision,
            DisplayDecision::NoUsablePhoto(NoUsablePhotoReason::ResourceNotCached)
        );
    }

    #[test]
    fn display_decision_refreshes_cached_plan_photo() {
        let snapshot = snapshot("a");
        let index = index(&["a"]);

        let decision =
            generate_display_decision_from_snapshot(&snapshot, &index, date("2026-06-08"), None);

        match decision {
            DisplayDecision::RefreshRequired {
                item,
                display_state,
                reason,
            } => {
                assert_eq!(item.image_sha256, "a");
                assert_eq!(display_state.image_sha256.as_deref(), Some("a"));
                assert_eq!(reason, RefreshReason::FirstBoot);
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn display_decision_uses_latest_past_plan_when_today_has_no_plan() {
        let snapshot = PlanSnapshot {
            content_hash: "hash-v1".to_string(),
            plans: vec![
                PlanItem {
                    date: date("2026-06-04"),
                    caption: "day-4".to_string(),
                    image_sha256: "4".to_string(),
                },
                PlanItem {
                    date: date("2026-06-07"),
                    caption: "day-7".to_string(),
                    image_sha256: "7".to_string(),
                },
                PlanItem {
                    date: date("2026-06-13"),
                    caption: "future".to_string(),
                    image_sha256: "13".to_string(),
                },
            ],
        };
        let index = index(&["7"]);

        let decision =
            generate_display_decision_from_snapshot(&snapshot, &index, date("2026-06-10"), None);

        match decision {
            DisplayDecision::RefreshRequired { item, .. } => {
                assert_eq!(item.date, date("2026-06-07"));
                assert_eq!(item.caption, "day-7");
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn display_decision_prefers_current_plan_over_past_plan() {
        let snapshot = PlanSnapshot {
            content_hash: "hash-v1".to_string(),
            plans: vec![
                PlanItem {
                    date: date("2026-06-07"),
                    caption: "day-7".to_string(),
                    image_sha256: "7".to_string(),
                },
                PlanItem {
                    date: date("2026-06-10"),
                    caption: "day-10".to_string(),
                    image_sha256: "10".to_string(),
                },
                PlanItem {
                    date: date("2026-06-12"),
                    caption: "future".to_string(),
                    image_sha256: "12".to_string(),
                },
            ],
        };
        let index = index(&["7", "10"]);

        let decision =
            generate_display_decision_from_snapshot(&snapshot, &index, date("2026-06-10"), None);

        match decision {
            DisplayDecision::RefreshRequired { item, .. } => {
                assert_eq!(item.date, date("2026-06-10"));
                assert_eq!(item.caption, "day-10");
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn display_decision_reports_no_plan_when_only_future_plans_exist() {
        let snapshot = PlanSnapshot {
            content_hash: "hash-v1".to_string(),
            plans: vec![PlanItem {
                date: date("2026-06-12"),
                caption: "future".to_string(),
                image_sha256: "12".to_string(),
            }],
        };

        let decision = generate_display_decision_from_snapshot(
            &snapshot,
            &ResourceIndex::default(),
            date("2026-06-10"),
            None,
        );

        assert_eq!(
            decision,
            DisplayDecision::NoUsablePhoto(NoUsablePhotoReason::NoPlanForDate)
        );
    }

    #[test]
    fn display_decision_sleeps_when_previous_state_matches() {
        let snapshot = snapshot("a");
        let index = index(&["a"]);
        let previous_display = DisplayState {
            plan_content_hash: Some("older-hash".to_string()),
            date: Some(date("2026-06-08")),
            image_sha256: Some("a".to_string()),
            caption: Some("caption".to_string()),
            refreshed_at_unix_secs: Some(100),
        };
        let mut previous_state = PersistentDeviceState::default();
        previous_state.set_current_display(&previous_display);

        let decision = generate_display_decision_from_snapshot(
            &snapshot,
            &index,
            date("2026-06-08"),
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
