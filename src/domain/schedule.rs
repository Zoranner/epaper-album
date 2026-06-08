use crate::model::{DisplayItem, DisplayState, LocalDate, PlanItem, PlanSnapshot};
use crate::power::BATTERY_SYNC_INTERVAL_SECONDS;

pub const DAILY_SYNC_INTERVAL_SECONDS: u64 = BATTERY_SYNC_INTERVAL_SECONDS;

pub fn next_daily_sync_epoch_seconds(
    last_successful_sync_epoch_seconds: u64,
    now_epoch_seconds: u64,
) -> u64 {
    let scheduled_epoch_seconds =
        last_successful_sync_epoch_seconds.saturating_add(DAILY_SYNC_INTERVAL_SECONDS);

    scheduled_epoch_seconds.max(now_epoch_seconds)
}

pub fn daily_sync_due(
    last_successful_sync_epoch_seconds: Option<u64>,
    now_epoch_seconds: u64,
) -> bool {
    let Some(last_successful_sync_epoch_seconds) = last_successful_sync_epoch_seconds else {
        return true;
    };

    now_epoch_seconds
        >= last_successful_sync_epoch_seconds.saturating_add(DAILY_SYNC_INTERVAL_SECONDS)
}

pub fn select_plan_for_date(plans: &[PlanItem], date: LocalDate) -> Option<&PlanItem> {
    plans
        .iter()
        .filter(|plan| plan.date <= date)
        .max_by_key(|plan| plan.date)
}

pub fn select_display_item(snapshot: &PlanSnapshot, date: LocalDate) -> Option<DisplayItem> {
    let plan = select_plan_for_date(&snapshot.plans, date)?;

    Some(DisplayItem {
        plan_content_hash: Some(snapshot.content_hash.clone()),
        date: plan.date,
        image_sha256: plan.image_sha256.clone(),
        caption: plan.caption.clone(),
    })
}

pub fn display_needs_refresh(previous: Option<&DisplayState>, next: &DisplayItem) -> bool {
    let Some(previous) = previous else {
        return true;
    };

    previous.date != Some(next.date)
        || previous.image_sha256.as_deref() != Some(next.image_sha256.as_str())
        || previous.caption.as_deref() != Some(next.caption.as_str())
}

pub fn next_plan_change_date(plans: &[PlanItem], date: LocalDate) -> Option<LocalDate> {
    plans
        .iter()
        .filter(|plan| plan.date > date)
        .map(|plan| plan.date)
        .min()
}

pub fn has_valid_plan_on_or_after(plans: &[PlanItem], date: LocalDate) -> bool {
    plans.iter().any(|plan| plan.date >= date)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(value: &str) -> LocalDate {
        LocalDate::parse(value).unwrap()
    }

    fn plan(date: &str, caption: &str, image_sha256: &str) -> PlanItem {
        PlanItem {
            date: self::date(date),
            caption: caption.to_string(),
            image_sha256: image_sha256.to_string(),
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
    fn exact_date_has_priority_over_past_plan() {
        let plans = vec![
            plan("2026-06-07", "day-7", "7"),
            plan("2026-06-08", "day-8", "8"),
            plan("2026-06-10", "day-10", "10"),
            plan("2026-06-12", "future", "12"),
        ];

        let selected = select_plan_for_date(&plans, date("2026-06-10")).unwrap();

        assert_eq!(selected.caption, "day-10");
    }

    #[test]
    fn does_not_select_future_plan_without_past_or_current_plan() {
        let plans = vec![
            plan("2026-06-12", "future-12", "12"),
            plan("2026-06-13", "future-13", "13"),
        ];

        assert!(select_plan_for_date(&plans, date("2026-06-10")).is_none());
    }

    #[test]
    fn builds_display_item_from_selected_plan() {
        let snapshot = PlanSnapshot {
            content_hash: "hash-v1".to_string(),
            plans: vec![plan("2026-06-06", "caption", "a")],
        };

        let item = select_display_item(&snapshot, date("2026-06-08")).unwrap();

        assert_eq!(item.plan_content_hash.as_deref(), Some("hash-v1"));
        assert_eq!(item.date, date("2026-06-06"));
        assert_eq!(item.image_sha256, "a");
    }

    #[test]
    fn detects_when_display_content_changed() {
        let next = DisplayItem {
            plan_content_hash: Some("hash-v2".to_string()),
            date: date("2026-06-06"),
            image_sha256: "hash".to_string(),
            caption: "caption".to_string(),
        };
        let previous = DisplayState {
            plan_content_hash: Some("hash-v1".to_string()),
            date: Some(date("2026-06-05")),
            image_sha256: Some("hash".to_string()),
            caption: Some("caption".to_string()),
            refreshed_at_unix_secs: Some(1),
        };

        assert!(display_needs_refresh(Some(&previous), &next));
    }

    #[test]
    fn ignores_plan_content_hash_when_display_content_is_unchanged() {
        let next = DisplayItem {
            plan_content_hash: Some("hash-v2".to_string()),
            date: date("2026-06-06"),
            image_sha256: "hash".to_string(),
            caption: "caption".to_string(),
        };
        let previous = DisplayState {
            plan_content_hash: Some("hash-v1".to_string()),
            date: Some(date("2026-06-06")),
            image_sha256: Some("hash".to_string()),
            caption: Some("caption".to_string()),
            refreshed_at_unix_secs: Some(1),
        };

        assert!(!display_needs_refresh(Some(&previous), &next));
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

    #[test]
    fn computes_next_daily_sync_from_last_successful_daily_sync() {
        assert_eq!(
            next_daily_sync_epoch_seconds(1_000, 10_000),
            1_000 + DAILY_SYNC_INTERVAL_SECONDS
        );
    }

    #[test]
    fn clamps_overdue_next_daily_sync_to_now() {
        assert_eq!(
            next_daily_sync_epoch_seconds(1_000, 1_000 + DAILY_SYNC_INTERVAL_SECONDS + 30),
            1_000 + DAILY_SYNC_INTERVAL_SECONDS + 30
        );
    }

    #[test]
    fn daily_sync_is_due_without_previous_success() {
        assert!(daily_sync_due(None, 10_000));
    }

    #[test]
    fn daily_sync_becomes_due_at_twenty_four_hours() {
        let last_successful_sync = 1_000;

        assert!(!daily_sync_due(
            Some(last_successful_sync),
            last_successful_sync + DAILY_SYNC_INTERVAL_SECONDS - 1
        ));
        assert!(daily_sync_due(
            Some(last_successful_sync),
            last_successful_sync + DAILY_SYNC_INTERVAL_SECONDS
        ));
    }
}
