use super::SyncErrorReport;
use crate::app::RunTrigger;
use crate::config::Config;
use crate::model::{LocalDate, Plan};
use crate::power::{BatteryStatus, PowerProfile};
use crate::state::{PersistentDeviceState, PersistentSyncState};

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

#[cfg(test)]
mod tests {
    use super::*;

    fn date(value: &str) -> LocalDate {
        LocalDate::parse(value).unwrap()
    }

    fn plan(date: &str, caption: &str, image: &str) -> Plan {
        Plan::fixed(self::date(date), caption, image)
    }

    fn context(plans: Option<Vec<Plan>>) -> RunContext {
        RunContext {
            now: 1,
            date: date("2026-06-08"),
            trigger: RunTrigger::Startup,
            battery: BatteryStatus::unknown(),
            power: PowerProfile::Battery,
            config: Some(crate::config::Config {
                wifi_ssid: "wifi".to_string(),
                wifi_password: "password".to_string(),
                base_url: "https://example.com".to_string(),
                secret_key: "secret".to_string(),
            }),
            plans,
            state: PersistentDeviceState::default(),
            sync: PersistentSyncState::default(),
        }
    }

    #[test]
    fn display_decision_reports_missing_config() {
        let mut context = context(None);
        context.config = None;

        let decision = decide_display(&context, |_| false, None);

        assert_eq!(decision.cause, DisplayCause::MissingConfig);
    }

    #[test]
    fn display_decision_reports_no_usable_photo_when_resource_is_missing() {
        let plans = vec![plan("2026-06-08", "caption", "a")];
        let context = context(Some(plans));

        let decision = decide_display(&context, |_| false, None);

        assert_eq!(decision.cause, DisplayCause::MissingPhoto);
    }

    #[test]
    fn display_decision_refreshes_cached_plan_photo() {
        let plans = vec![plan("2026-06-08", "caption", "a")];
        let context = context(Some(plans));

        let decision = decide_display(&context, |image| image == "a", None);

        match decision.action {
            DisplayAction::Refresh(DisplayTarget::Photo { image, .. }) => {
                assert_eq!(image, "a");
                assert_eq!(decision.cause, DisplayCause::First);
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
        let mut context = context(Some(plans));
        context.date = date("2026-06-10");

        let decision = decide_display(&context, |image| image == "7", None);

        match decision.action {
            DisplayAction::Refresh(DisplayTarget::Photo { caption, .. }) => {
                assert_eq!(caption, "day-7");
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
        let mut context = context(Some(plans));
        context.date = date("2026-06-10");

        let decision = decide_display(&context, |image| image == "12", None);

        match decision.action {
            DisplayAction::Refresh(DisplayTarget::Photo { image, .. }) => {
                assert_eq!(image, "12");
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn display_decision_sleeps_when_previous_state_matches() {
        let plans = vec![plan("2026-06-08", "caption", "a")];
        let mut context = context(Some(plans.clone()));
        context.state = PersistentDeviceState::from_display(date("2026-06-08"), &plans[0]);

        let decision = decide_display(&context, |image| image == "a", None);

        assert_eq!(decision.action, DisplayAction::Keep);
    }
}
