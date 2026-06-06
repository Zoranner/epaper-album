use crate::model::{CloudPlanResponse, DisplayItem, DisplayState, LocalDate, PlanItem};

pub fn select_plan_for_date(plans: &[PlanItem], date: LocalDate) -> Option<&PlanItem> {
    plans.iter().find(|plan| plan.contains_date(date))
}

pub fn select_display_item(
    response: &CloudPlanResponse,
    date: LocalDate,
    rotation_slot: u64,
) -> Option<DisplayItem> {
    let plan = select_plan_for_date(&response.plans, date)?;
    let (image_index, image_sha256) = plan.image_at_slot(rotation_slot)?;

    Some(DisplayItem {
        plan_version: response.version.clone(),
        date,
        image_sha256: image_sha256.to_string(),
        image_index,
        caption: plan.caption.clone(),
    })
}

pub fn display_needs_refresh(previous: Option<&DisplayState>, next: &DisplayItem) -> bool {
    let Some(previous) = previous else {
        return true;
    };

    previous.plan_version.as_deref() != Some(next.plan_version.as_str())
        || previous.date != Some(next.date)
        || previous.image_sha256.as_deref() != Some(next.image_sha256.as_str())
        || previous.image_index != next.image_index
        || previous.caption.as_deref() != Some(next.caption.as_str())
}

pub fn next_plan_change_date(plans: &[PlanItem], date: LocalDate) -> Option<LocalDate> {
    plans
        .iter()
        .filter_map(|plan| {
            if plan.start > date {
                return Some(plan.start);
            }

            if plan.contains_date(date) {
                return Some(plan.end.next_day());
            }

            None
        })
        .filter(|candidate| *candidate > date)
        .min()
}

pub fn has_valid_plan_on_or_after(plans: &[PlanItem], date: LocalDate) -> bool {
    plans.iter().any(|plan| plan.end >= date)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(value: &str) -> LocalDate {
        LocalDate::parse(value).unwrap()
    }

    fn plan(start: &str, end: &str, caption: &str, images: &[&str]) -> PlanItem {
        PlanItem {
            start: date(start),
            end: date(end),
            caption: caption.to_string(),
            images: images.iter().map(|image| image.to_string()).collect(),
        }
    }

    #[test]
    fn selects_plan_by_inclusive_date_range() {
        let plans = vec![
            plan("2026-06-01", "2026-06-05", "old", &["old"]),
            plan("2026-06-06", "2026-06-08", "today", &["today"]),
        ];

        let selected = select_plan_for_date(&plans, date("2026-06-06")).unwrap();

        assert_eq!(selected.caption, "today");
    }

    #[test]
    fn selects_image_by_rotation_slot() {
        let response = CloudPlanResponse {
            version: "v1".to_string(),
            plans: vec![plan("2026-06-06", "2026-06-06", "caption", &["a", "b"])],
        };

        let item = select_display_item(&response, date("2026-06-06"), 3).unwrap();

        assert_eq!(item.image_index, 1);
        assert_eq!(item.image_sha256, "b");
    }

    #[test]
    fn detects_when_display_content_changed() {
        let next = DisplayItem {
            plan_version: "v2".to_string(),
            date: date("2026-06-06"),
            image_sha256: "hash".to_string(),
            image_index: 0,
            caption: "caption".to_string(),
        };
        let previous = DisplayState {
            plan_version: Some("v1".to_string()),
            date: Some(date("2026-06-06")),
            image_sha256: Some("hash".to_string()),
            image_index: 0,
            caption: Some("caption".to_string()),
            refreshed_at_unix_secs: Some(1),
        };

        assert!(display_needs_refresh(Some(&previous), &next));
    }

    #[test]
    fn computes_next_plan_change_from_current_plan_end() {
        let plans = vec![
            plan("2026-06-06", "2026-06-08", "current", &["a"]),
            plan("2026-06-10", "2026-06-10", "future", &["b"]),
        ];

        assert_eq!(
            next_plan_change_date(&plans, date("2026-06-06")),
            Some(date("2026-06-09"))
        );
    }
}
