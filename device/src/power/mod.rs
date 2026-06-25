mod battery;
mod schedule;
mod sleep;
mod wake;

#[cfg(target_os = "espidf")]
pub mod espidf;

pub use battery::{BatteryStatus, ChargeState, LowBatteryPolicy, PowerProfile};
pub use schedule::{
    local_date_start_epoch_seconds, next_beijing_hour_epoch_seconds, next_power_run_epoch_seconds,
    next_run_plan, NextRunPlan, BEIJING_UTC_OFFSET_SECONDS, DAILY_SECONDS,
    HOURLY_RUN_INTERVAL_SECONDS,
};
pub use sleep::{battery_deep_sleep_wake_policy, DeepSleepWakePolicy};
pub use wake::WakeProbe;
