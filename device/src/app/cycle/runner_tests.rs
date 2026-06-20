use super::test_support::{date, input, plan, FakeDisplay, FakeError, FakeSync};
use super::{
    run_device_cycle, DeviceCycleOutcome, DisplayCause, SpriteSet, SyncAction, SyncCause,
    SyncErrorReport, SyncResult,
};
use crate::model::Plan;
use crate::state::PersistentDeviceState;

#[test]
fn sync_updates_plans_and_refreshes_display() {
    let remote_plans = vec![plan("a")];
    let mut sync = FakeSync {
        result: Some(Ok(SyncResult {
            plans: remote_plans.clone(),
            sprites: SpriteSet::default(),
        })),
        requests: Vec::new(),
    };
    let mut display = FakeDisplay {
        images: vec!["a".to_string()],
        ..FakeDisplay::default()
    };

    let result = run_device_cycle(input(None), &mut sync, &mut display);

    assert_eq!(sync.requests.len(), 1);
    assert_eq!(result.sync_decision.action, SyncAction::Fetch);
    assert_eq!(result.sync_decision.cause, SyncCause::Daily);
    assert_eq!(display.requests.len(), 1);
    assert_eq!(display.requests[0].plan.image, "a");
    assert!(result.sync_succeeded);
    assert!(result.refresh_succeeded);
    assert_eq!(result.persistent_state.image.as_deref(), Some("a"));
}

#[test]
fn low_battery_skips_sync_but_refreshes_current_state() {
    let plans = vec![plan("a")];
    let mut input = input(Some(plans));
    input.persistent_state = PersistentDeviceState::from_display(input.date, &plan("a"));
    input.battery.low_battery = true;
    let mut sync = FakeSync::default();
    let mut display = FakeDisplay {
        images: vec!["a".to_string()],
        ..FakeDisplay::default()
    };

    let result = run_device_cycle(input, &mut sync, &mut display);

    assert!(sync.requests.is_empty());
    assert_eq!(result.sync_decision.action, SyncAction::Skip);
    assert_eq!(result.sync_decision.cause, SyncCause::LowBattery);
    assert!(display.requests.is_empty());
    assert_eq!(display.error_requests.len(), 1);
    assert_eq!(display.error_requests[0].title, "LOW BATTERY");
    assert_eq!(result.outcome, DeviceCycleOutcome::LowBatterySkipSync);
    assert_eq!(result.persistent_state.image, None);
}

#[test]
fn low_battery_refreshes_error_page_when_photo_state_matches() {
    let plans = vec![plan("a")];
    let mut input = input(Some(plans));
    input.battery.low_battery = true;
    input.persistent_state = PersistentDeviceState::from_display(input.date, &plan("a"));
    let mut sync = FakeSync::default();
    let mut display = FakeDisplay {
        images: vec!["a".to_string()],
        ..FakeDisplay::default()
    };

    let result = run_device_cycle(input, &mut sync, &mut display);

    assert!(sync.requests.is_empty());
    assert_eq!(result.sync_decision.action, SyncAction::Skip);
    assert_eq!(result.sync_decision.cause, SyncCause::LowBattery);
    assert!(display.requests.is_empty());
    assert_eq!(display.error_requests.len(), 1);
    assert_eq!(display.error_requests[0].title, "LOW BATTERY");
    assert_eq!(result.outcome, DeviceCycleOutcome::LowBatterySkipSync);
    assert!(result.display_available);
}

#[test]
fn keep_decision_marks_display_available_when_state_was_loaded() {
    let plans = vec![plan("a")];
    let mut input = input(Some(plans));
    input.persistent_state_loaded = true;
    input.sync_state.date = Some(input.date);
    input.persistent_state = PersistentDeviceState::from_display(input.date, &plan("a"));
    let mut sync = FakeSync::default();
    let mut display = FakeDisplay {
        images: vec!["a".to_string()],
        ..FakeDisplay::default()
    };

    let result = run_device_cycle(input, &mut sync, &mut display);

    assert!(sync.requests.is_empty());
    assert!(display.requests.is_empty());
    assert!(result.display_available);
}

#[test]
fn charging_low_battery_status_still_syncs() {
    let mut input = input(Some(vec![plan("a")]));
    input.battery =
        crate::power::BatteryStatus::new(0, Some(0), crate::power::ChargeState::Charging, true);
    let mut sync = FakeSync {
        result: Some(Ok(SyncResult {
            plans: vec![plan("a")],
            sprites: SpriteSet::default(),
        })),
        requests: Vec::new(),
    };
    let mut display = FakeDisplay {
        images: vec!["a".to_string()],
        ..FakeDisplay::default()
    };

    let result = run_device_cycle(input, &mut sync, &mut display);

    assert_eq!(sync.requests.len(), 1);
    assert_eq!(result.sync_decision.action, SyncAction::Fetch);
    assert_eq!(result.sync_decision.cause, SyncCause::External);
    assert!(result.sync_attempted);
    assert!(result.sync_succeeded);
    assert_ne!(result.outcome, DeviceCycleOutcome::LowBatterySkipSync);
}

#[test]
fn charging_keeps_screen_when_photo_is_unchanged_after_sync() {
    let mut input = input(Some(vec![plan("a")]));
    input.persistent_state_loaded = true;
    input.persistent_state = PersistentDeviceState::from_display(input.date, &plan("a"));
    input.battery =
        crate::power::BatteryStatus::new(0, Some(100), crate::power::ChargeState::Full, false);
    let mut sync = FakeSync {
        result: Some(Ok(SyncResult {
            plans: vec![plan("a")],
            sprites: SpriteSet::default(),
        })),
        requests: Vec::new(),
    };
    let mut display = FakeDisplay {
        images: vec!["a".to_string()],
        ..FakeDisplay::default()
    };

    let result = run_device_cycle(input, &mut sync, &mut display);

    assert_eq!(sync.requests.len(), 1);
    assert_eq!(result.sync_decision.action, SyncAction::Fetch);
    assert_eq!(result.sync_decision.cause, SyncCause::External);
    assert!(display.requests.is_empty());
    assert!(display.error_requests.is_empty());
    assert_eq!(result.display_decision.cause, DisplayCause::Same);
}

#[test]
fn sync_failure_without_displayable_cache_refreshes_error_page() {
    let mut sync = FakeSync {
        result: Some(Err(FakeError("network down"))),
        requests: Vec::new(),
    };
    let mut display = FakeDisplay::default();

    let result = run_device_cycle(input(Some(vec![plan("a")])), &mut sync, &mut display);

    assert_eq!(sync.requests.len(), 1);
    assert!(display.requests.is_empty());
    assert_eq!(display.error_requests.len(), 1);
    assert_eq!(display.error_requests[0].title, "SYNC ERROR");
    assert_eq!(display.error_requests[0].detail, "network down");
    assert_eq!(
        result.sync_error_report,
        Some(SyncErrorReport::new(
            "sync.error",
            "sync",
            None,
            "CANNOT UPDATE SERVER DATA",
            "network down"
        ))
    );
    assert_eq!(result.outcome, DeviceCycleOutcome::SyncFailed);
}

#[test]
fn sync_failure_refreshes_error_page_even_when_photo_is_usable() {
    let mut input = input(Some(vec![plan("a")]));
    input.persistent_state_loaded = true;
    input.persistent_state = PersistentDeviceState::from_display(input.date, &plan("a"));
    input.sync_state.date = Some(date("2026-06-07"));
    let mut sync = FakeSync {
        result: Some(Err(FakeError("network down"))),
        requests: Vec::new(),
    };
    let mut display = FakeDisplay {
        images: vec!["a".to_string()],
        ..FakeDisplay::default()
    };

    let result = run_device_cycle(input, &mut sync, &mut display);

    assert_eq!(sync.requests.len(), 1);
    assert!(display.requests.is_empty());
    assert_eq!(display.error_requests.len(), 1);
    assert_eq!(display.error_requests[0].title, "SYNC ERROR");
    assert_eq!(display.error_requests[0].detail, "network down");
    assert_eq!(result.display_decision.cause, DisplayCause::Sync);
    assert_eq!(result.persistent_state.image, None);
    assert_eq!(result.outcome, DeviceCycleOutcome::SyncFailed);
}

#[test]
fn missing_today_plan_keeps_latest_past_plan_without_sync_error() {
    let plans = vec![Plan::fixed(date("2026-06-16"), "past", "a")];
    let mut input = input(Some(plans.clone()));
    input.date = date("2026-06-18");
    input.sync_state.date = Some(date("2026-06-18"));
    let mut sync = FakeSync::default();
    let mut display = FakeDisplay {
        images: vec!["a".to_string()],
        ..FakeDisplay::default()
    };

    let result = run_device_cycle(input, &mut sync, &mut display);

    assert!(sync.requests.is_empty());
    assert_eq!(display.requests.len(), 1);
    assert!(display.error_requests.is_empty());
    assert_eq!(display.requests[0].date, date("2026-06-18"));
    assert_eq!(display.requests[0].plan.image, "a");
    assert_eq!(result.display_decision.cause, DisplayCause::First);
    assert_ne!(result.outcome, DeviceCycleOutcome::SyncFailed);
}

#[test]
fn sync_success_keeps_screen_when_photo_is_unchanged() {
    let mut input = input(Some(vec![plan("a")]));
    input.persistent_state_loaded = true;
    input.persistent_state = PersistentDeviceState::from_display(input.date, &plan("a"));
    input.sync_state.date = Some(date("2026-06-07"));
    let mut sync = FakeSync {
        result: Some(Ok(SyncResult {
            plans: vec![plan("a")],
            sprites: SpriteSet::default(),
        })),
        requests: Vec::new(),
    };
    let mut display = FakeDisplay {
        images: vec!["a".to_string()],
        ..FakeDisplay::default()
    };

    let result = run_device_cycle(input, &mut sync, &mut display);

    assert_eq!(sync.requests.len(), 1);
    assert!(result.sync_succeeded);
    assert!(display.requests.is_empty());
    assert!(display.error_requests.is_empty());
    assert_eq!(result.display_decision.cause, DisplayCause::Same);
    assert_eq!(result.outcome, DeviceCycleOutcome::SyncRequested);
}

#[test]
fn battery_skips_sync_when_plan_already_synced_today() {
    let mut input = input(Some(vec![plan("a")]));
    input.sync_state.date = Some(date("2026-06-08"));
    input.persistent_state = PersistentDeviceState::from_display(date("2026-06-08"), &plan("a"));
    let mut sync = FakeSync::default();
    let mut display = FakeDisplay {
        images: vec!["a".to_string()],
        ..FakeDisplay::default()
    };

    let result = run_device_cycle(input, &mut sync, &mut display);

    assert!(sync.requests.is_empty());
    assert_eq!(result.sync_decision.action, SyncAction::Skip);
    assert_eq!(result.sync_decision.cause, SyncCause::Done);
    assert!(!result.sync_attempted);
}

#[test]
fn battery_requests_sync_when_plan_not_synced_today() {
    let mut input = input(Some(vec![plan("a")]));
    input.sync_state.date = Some(date("2026-06-07"));
    let mut sync = FakeSync {
        result: Some(Ok(SyncResult {
            plans: vec![plan("a")],
            sprites: SpriteSet::default(),
        })),
        requests: Vec::new(),
    };
    let mut display = FakeDisplay {
        images: vec!["a".to_string()],
        ..FakeDisplay::default()
    };

    let result = run_device_cycle(input, &mut sync, &mut display);

    assert_eq!(sync.requests.len(), 1);
    assert_eq!(result.sync_decision.action, SyncAction::Fetch);
    assert_eq!(result.sync_decision.cause, SyncCause::Daily);
    assert!(result.sync_attempted);
    assert!(result.sync_succeeded);
    assert_eq!(result.sync_state.date, Some(date("2026-06-08")));
}

#[test]
fn external_power_syncs_but_keeps_screen_when_display_is_unchanged() {
    let plans = vec![plan("a")];
    let mut input = input(Some(plans.clone()));
    input.persistent_state_loaded = true;
    input.persistent_state = PersistentDeviceState::from_display(input.date, &plans[0]);
    input.battery =
        crate::power::BatteryStatus::new(0, Some(100), crate::power::ChargeState::Full, false);
    let mut sync = FakeSync {
        result: Some(Ok(SyncResult {
            plans,
            sprites: SpriteSet::default(),
        })),
        requests: Vec::new(),
    };
    let mut display = FakeDisplay {
        images: vec!["a".to_string()],
        ..FakeDisplay::default()
    };

    let result = run_device_cycle(input, &mut sync, &mut display);

    assert_eq!(result.sync_decision.action, SyncAction::Fetch);
    assert_eq!(result.sync_decision.cause, SyncCause::External);
    assert!(result.sync_attempted);
    assert!(result.sync_succeeded);
    assert!(display.requests.is_empty());
    assert_eq!(result.display_decision.cause, DisplayCause::Same);
    assert_eq!(result.outcome, DeviceCycleOutcome::SyncRequested);
    assert!(result.display_available);
}
