pub mod cycle;

use crate::state::WakeReason;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device_runtime::{
        decide_display, DisplayAction, DisplayCause, DisplayTarget, RunContext,
    };
    use crate::model::{LocalDate, Plan};
    use crate::power::{BatteryStatus, PowerProfile};
    use crate::state::PersistentDeviceState;

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
            sync: crate::state::PersistentSyncState::default(),
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
