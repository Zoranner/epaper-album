use crate::model::{LocalDate, Plan};
use crate::state::PersistentDeviceState;

pub fn select_plan_for_date(plans: &[Plan], date: LocalDate) -> Option<&Plan> {
    plans
        .iter()
        .filter(|plan| plan.date <= date)
        .max_by_key(|plan| plan.date)
        .or_else(|| {
            plans
                .iter()
                .filter(|plan| plan.date > date)
                .min_by_key(|plan| plan.date)
        })
}

pub fn display_needs_refresh(previous: &PersistentDeviceState, next: &Plan) -> bool {
    !previous.matches_plan(next)
}

pub fn next_plan_change_date(plans: &[Plan], date: LocalDate) -> Option<LocalDate> {
    plans
        .iter()
        .filter(|plan| plan.date > date)
        .map(|plan| plan.date)
        .min()
}

pub fn has_plan_on_or_after(plans: &[Plan], date: LocalDate) -> bool {
    plans.iter().any(|plan| plan.date >= date)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn selects_plan_by_exact_date() {
        let plans = vec![
            plan("2026-06-01", "old", "old"),
            plan("2026-06-06", "today", "today"),
        ];

        let selected = select_plan_for_date(&plans, date("2026-06-06")).unwrap();

        assert_eq!(selected.caption, "today");
    }

    #[test]
    fn selects_latest_past_plan_when_date_has_no_plan() {
        let plans = vec![
            plan("2026-06-04", "day-4", "4"),
            plan("2026-06-05", "day-5", "5"),
            plan("2026-06-07", "day-7", "7"),
            plan("2026-06-13", "future", "13"),
        ];

        let selected = select_plan_for_date(&plans, date("2026-06-10")).unwrap();

        assert_eq!(selected.caption, "day-7");
    }

    #[test]
    fn selects_nearest_future_plan_when_no_past_plan_exists() {
        let plans = vec![
            plan("2026-06-12", "future-12", "12"),
            plan("2026-06-13", "future-13", "13"),
        ];

        let selected = select_plan_for_date(&plans, date("2026-06-10")).unwrap();

        assert_eq!(selected.caption, "future-12");
    }

    #[test]
    fn detects_when_display_content_changed() {
        let next = plan("2026-06-06", "caption", "hash");
        let previous = PersistentDeviceState {
            date: Some(date("2026-06-05")),
            image: Some("hash".to_string()),
            caption: Some("caption".to_string()),
            notice: None,
        };

        assert!(display_needs_refresh(&previous, &next));
    }

    #[test]
    fn skips_refresh_when_display_content_is_unchanged() {
        let next = plan("2026-06-06", "caption", "hash");
        let previous = PersistentDeviceState::from_plan(&next, None);

        assert!(!display_needs_refresh(&previous, &next));
    }

    #[test]
    fn computes_next_plan_change_from_next_plan_date() {
        let plans = vec![
            plan("2026-06-06", "current", "a"),
            plan("2026-06-10", "future", "b"),
        ];

        assert_eq!(
            next_plan_change_date(&plans, date("2026-06-06")),
            Some(date("2026-06-10"))
        );
    }
}
