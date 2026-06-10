pub mod cycle;

use crate::power::BatteryStatus;
use crate::schedule::{display_needs_refresh, select_plan_for_date};
use crate::state::{PersistedDisplay, PersistentDeviceState, RefreshReason, WakeReason};
use crate::{model::LocalDate, model::Plan};

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
    RefreshRequired { plan: Plan, reason: RefreshReason },
    SleepOnly { plan: Plan },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NoUsablePhotoReason {
    NoPlan,
    ResourceNotCached,
}

pub fn generate_display_decision(
    plans: Option<&[Plan]>,
    image_exists: impl FnMut(&str) -> bool,
    date: LocalDate,
    previous_state: Option<&PersistentDeviceState>,
) -> DisplayDecision {
    let Some(plans) = plans else {
        return DisplayDecision::MissingConfig;
    };

    generate_display_decision_from_plans(plans, image_exists, date, previous_state)
}

pub fn generate_display_decision_from_plans(
    plans: &[Plan],
    mut image_exists: impl FnMut(&str) -> bool,
    date: LocalDate,
    previous_state: Option<&PersistentDeviceState>,
) -> DisplayDecision {
    let Some(plan) = select_plan_for_date(plans, date) else {
        return DisplayDecision::NoUsablePhoto(NoUsablePhotoReason::NoPlan);
    };

    if !image_exists(&plan.image) {
        return DisplayDecision::NoUsablePhoto(NoUsablePhotoReason::ResourceNotCached);
    }

    let empty_display = PersistedDisplay::default();
    let previous_display = previous_state
        .map(|state| &state.current_display)
        .unwrap_or(&empty_display);

    if display_needs_refresh(previous_display, plan) {
        let reason = if previous_display.image.is_some() {
            RefreshReason::PlanChanged
        } else {
            RefreshReason::FirstBoot
        };

        return DisplayDecision::RefreshRequired {
            plan: plan.clone(),
            reason,
        };
    }

    DisplayDecision::SleepOnly { plan: plan.clone() }
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

    fn date(value: &str) -> LocalDate {
        LocalDate::parse(value).unwrap()
    }

    fn plan(date: &str, caption: &str, image: &str) -> Plan {
        Plan {
            date: self::date(date),
            caption: caption.to_string(),
            image: image.to_string(),
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

    #[test]
    fn display_decision_reports_missing_config() {
        let decision = generate_display_decision(None, |_| false, date("2026-06-08"), None);

        assert_eq!(decision, DisplayDecision::MissingConfig);
    }

    #[test]
    fn display_decision_reports_no_usable_photo_when_resource_is_missing() {
        let plans = vec![plan("2026-06-08", "caption", "a")];

        let decision =
            generate_display_decision_from_plans(&plans, |_| false, date("2026-06-08"), None);

        assert_eq!(
            decision,
            DisplayDecision::NoUsablePhoto(NoUsablePhotoReason::ResourceNotCached)
        );
    }

    #[test]
    fn display_decision_refreshes_cached_plan_photo() {
        let plans = vec![plan("2026-06-08", "caption", "a")];

        let decision = generate_display_decision_from_plans(
            &plans,
            |image| image == "a",
            date("2026-06-08"),
            None,
        );

        match decision {
            DisplayDecision::RefreshRequired { plan, reason } => {
                assert_eq!(plan.image, "a");
                assert_eq!(reason, RefreshReason::FirstBoot);
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn display_decision_uses_latest_past_plan_when_today_has_no_plan() {
        let plans = vec![
            plan("2026-06-04", "day-4", "4"),
            plan("2026-06-07", "day-7", "7"),
            plan("2026-06-13", "future", "13"),
        ];

        let decision = generate_display_decision_from_plans(
            &plans,
            |image| image == "7",
            date("2026-06-10"),
            None,
        );

        match decision {
            DisplayDecision::RefreshRequired { plan, .. } => {
                assert_eq!(plan.date, date("2026-06-07"));
                assert_eq!(plan.caption, "day-7");
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn display_decision_uses_nearest_future_plan_when_no_past_plan_exists() {
        let plans = vec![
            plan("2026-06-12", "future-12", "12"),
            plan("2026-06-13", "future-13", "13"),
        ];

        let decision = generate_display_decision_from_plans(
            &plans,
            |image| image == "12",
            date("2026-06-10"),
            None,
        );

        match decision {
            DisplayDecision::RefreshRequired { plan, .. } => {
                assert_eq!(plan.date, date("2026-06-12"));
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn display_decision_sleeps_when_previous_state_matches() {
        let plans = vec![plan("2026-06-08", "caption", "a")];
        let mut previous_state = PersistentDeviceState::default();
        previous_state.set_current_display(&plans[0]);

        let decision = generate_display_decision_from_plans(
            &plans,
            |image| image == "a",
            date("2026-06-08"),
            Some(&previous_state),
        );

        match decision {
            DisplayDecision::SleepOnly { plan } => {
                assert_eq!(plan.image, "a");
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }
}
