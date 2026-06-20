use crate::device_runtime::DeviceCycleResult;
use crate::model::LocalDate;
use crate::power::{next_power_run_epoch_seconds, next_run_plan, NextRunPlan, PowerProfile};

pub fn build_next_run_plan(cycle: &DeviceCycleResult, now_epoch_seconds: u64) -> NextRunPlan {
    let power_profile = PowerProfile::from(&cycle.battery);
    let next_power_run = next_power_run_epoch_seconds(now_epoch_seconds, power_profile);

    next_run_plan(now_epoch_seconds, next_power_run, None, None)
}

pub fn current_epoch_seconds() -> u64 {
    chrono::Utc::now().timestamp().max(0) as u64
}

pub fn today() -> LocalDate {
    use chrono::{Datelike, TimeZone, Utc};

    let timestamp = chrono::Utc::now()
        .timestamp()
        .saturating_add(8 * 60 * 60)
        .max(0);
    let now = Utc
        .timestamp_opt(timestamp, 0)
        .single()
        .unwrap_or_else(Utc::now);
    LocalDate::new(now.year() as u16, now.month() as u8, now.day() as u8)
        .unwrap_or_else(|| LocalDate::parse("2026-01-01").unwrap())
}
