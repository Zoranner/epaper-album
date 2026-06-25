#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeepSleepWakePolicy {
    TimerOnly,
    TimerAndPmicIrq,
}

impl DeepSleepWakePolicy {
    pub const fn label(self) -> &'static str {
        match self {
            Self::TimerOnly => "timer-only",
            Self::TimerAndPmicIrq => "timer-and-pmic-irq",
        }
    }

    pub const fn uses_pmic_irq(self) -> bool {
        matches!(self, Self::TimerAndPmicIrq)
    }
}

pub const fn battery_deep_sleep_wake_policy(
    externally_powered: bool,
) -> Option<DeepSleepWakePolicy> {
    if externally_powered {
        None
    } else {
        Some(DeepSleepWakePolicy::TimerOnly)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn battery_power_uses_timer_only_deep_sleep() {
        assert_eq!(
            battery_deep_sleep_wake_policy(false),
            Some(DeepSleepWakePolicy::TimerOnly)
        );
        assert!(!battery_deep_sleep_wake_policy(false)
            .expect("battery mode has a sleep policy")
            .uses_pmic_irq());
    }

    #[test]
    fn external_power_does_not_enter_deep_sleep() {
        assert_eq!(battery_deep_sleep_wake_policy(true), None);
    }
}
