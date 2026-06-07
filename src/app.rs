use crate::power::BatteryStatus;
use crate::state::{PersistentDeviceState, WakeReason};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunTrigger {
    Startup,
    Wake(WakeReason),
    Manual,
}

impl RunTrigger {
    pub const fn wake_reason(self) -> WakeReason {
        match self {
            Self::Startup => WakeReason::Startup,
            Self::Wake(reason) => reason,
            Self::Manual => WakeReason::ManualButton,
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
    pub force_sync: bool,
    pub daily_sync_consumed: bool,
    pub display_refresh_due: bool,
    pub battery: BatteryStatus,
    pub persistent_state: PersistentDeviceState,
}

pub fn run_once(mut input: RunInput) -> RunReport {
    let wake_reason = input.trigger.wake_reason();
    input.persistent_state.last_wake_reason = Some(wake_reason);

    let force_sync = matches!(input.trigger, RunTrigger::Manual);
    let sync_requested = force_sync || input.daily_sync_due;
    let daily_sync_consumed = input.daily_sync_due && !force_sync && !input.battery.low_battery;

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
        force_sync,
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

    #[test]
    fn manual_run_forces_sync_without_consuming_daily_sync() {
        let mut input = input(RunTrigger::Manual);
        input.daily_sync_due = true;

        let report = run_once(input);

        assert_eq!(report.outcome, RunOutcome::SyncRequested);
        assert!(report.force_sync);
        assert!(!report.daily_sync_consumed);
        assert_eq!(report.wake_reason, WakeReason::ManualButton);
        assert_eq!(
            report.persistent_state.last_wake_reason,
            Some(WakeReason::ManualButton)
        );
    }

    #[test]
    fn timer_wake_consumes_due_daily_sync() {
        let mut input = input(RunTrigger::Wake(WakeReason::Timer));
        input.daily_sync_due = true;

        let report = run_once(input);

        assert_eq!(report.outcome, RunOutcome::SyncRequested);
        assert!(!report.force_sync);
        assert!(report.daily_sync_consumed);
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
}
