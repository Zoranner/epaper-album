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
    let page = sync_error_page_fields(sync_error);
    DisplayDecision {
        action: DisplayAction::Refresh(DisplayTarget::Page {
            date,
            title: page.title,
            message: page.message,
            hint: page.hint,
            detail: page.detail,
        }),
        cause: DisplayCause::Sync,
    }
}

struct SyncErrorPageFields {
    title: String,
    message: String,
    hint: String,
    detail: String,
}

fn sync_error_page_fields(sync_error: &SyncErrorReport) -> SyncErrorPageFields {
    let stage = sync_error
        .stage
        .as_deref()
        .unwrap_or(sync_error.category.as_str());
    let detail = sync_error.detail.as_str();

    let title = if sync_error.category == "resource" {
        "RESOURCE ERROR"
    } else {
        "SYNC ERROR"
    };
    let message = format!("STAGE: {}", stage.to_ascii_uppercase());
    let hint = http_status_hint(detail).unwrap_or_else(|| {
        if stage == "sprite caption" || stage == "sprite date" {
            "CHECK SERVER SPRITE/FONTS".to_string()
        } else {
            "CHECK WIFI BASE URL AND SERVER".to_string()
        }
    });
    let detail = if stage == "sprite caption" || stage == "sprite date" {
        "CHECK SERVER SPRITE/FONTS".to_string()
    } else {
        sync_error.detail.clone()
    };

    SyncErrorPageFields {
        title: ascii_safe(title),
        message: ascii_safe(&message),
        hint: ascii_safe(&hint),
        detail: ascii_safe(&detail),
    }
}

fn http_status_hint(detail: &str) -> Option<String> {
    let status = detail.strip_prefix("cloud: http-status-")?;
    if status.len() == 3 && status.bytes().all(|byte| byte.is_ascii_digit()) {
        Some(format!("HTTP {status} FROM SERVER"))
    } else {
        None
    }
}

fn ascii_safe(value: &str) -> String {
    value
        .chars()
        .map(
            |character| {
                if character.is_ascii() {
                    character
                } else {
                    '?'
                }
            },
        )
        .collect()
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

    #[test]
    fn sync_resource_error_page_uses_ascii_diagnostic_fields() {
        let caption = "\u{53ef}\u{53ef}\u{7231}\u{7231}\u{5c0f}\u{4e1b}\u{4e1b}";
        let context = context(Some(vec![plan("2026-06-08", caption, "a")]));
        let report = SyncErrorReport::new(
            "resource.cloud.http-status",
            "resource",
            Some("sprite caption".to_string()),
            "sprite caption sync failed",
            "cloud: http-status-500",
        );

        let decision = decide_display(&context, |image| image == "a", Some(&report));

        match decision.action {
            DisplayAction::Refresh(DisplayTarget::Page {
                title,
                message,
                hint,
                detail,
                ..
            }) => {
                assert_eq!(title, "RESOURCE ERROR");
                assert_eq!(message, "STAGE: SPRITE CAPTION");
                assert_eq!(hint, "HTTP 500 FROM SERVER");
                assert_eq!(detail, "CHECK SERVER SPRITE/FONTS");
                assert!(title.is_ascii());
                assert!(message.is_ascii());
                assert!(hint.is_ascii());
                assert!(detail.is_ascii());
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }
}
