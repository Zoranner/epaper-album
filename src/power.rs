#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChargeState {
    Unknown,
    Discharging,
    Charging,
    Full,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatteryStatus {
    pub millivolts: u16,
    pub percent: Option<u8>,
    pub charge_state: ChargeState,
    pub low_battery: bool,
}

impl BatteryStatus {
    pub const fn new(
        millivolts: u16,
        percent: Option<u8>,
        charge_state: ChargeState,
        low_battery: bool,
    ) -> Self {
        Self {
            millivolts,
            percent,
            charge_state,
            low_battery,
        }
    }

    pub const fn unknown() -> Self {
        Self {
            millivolts: 0,
            percent: None,
            charge_state: ChargeState::Unknown,
            low_battery: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LowBatteryPolicy {
    pub stop_cloud_sync: bool,
    pub show_notice: bool,
    pub minimum_percent: Option<u8>,
    pub minimum_millivolts: Option<u16>,
}

impl Default for LowBatteryPolicy {
    fn default() -> Self {
        Self {
            stop_cloud_sync: true,
            show_notice: true,
            minimum_percent: Some(15),
            minimum_millivolts: None,
        }
    }
}

impl LowBatteryPolicy {
    pub fn is_low_battery(&self, battery: &BatteryStatus) -> bool {
        if battery.low_battery {
            return true;
        }

        if let (Some(percent), Some(minimum_percent)) = (battery.percent, self.minimum_percent) {
            if percent <= minimum_percent {
                return true;
            }
        }

        if let Some(minimum_millivolts) = self.minimum_millivolts {
            if battery.millivolts != 0 && battery.millivolts <= minimum_millivolts {
                return true;
            }
        }

        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SleepPlan {
    pub next_wakeup_epoch_seconds: Option<u64>,
    pub deep_sleep_seconds: Option<u32>,
}

impl SleepPlan {
    pub const fn wake_at(next_wakeup_epoch_seconds: u64) -> Self {
        Self {
            next_wakeup_epoch_seconds: Some(next_wakeup_epoch_seconds),
            deep_sleep_seconds: None,
        }
    }

    pub const fn sleep_for(deep_sleep_seconds: u32) -> Self {
        Self {
            next_wakeup_epoch_seconds: None,
            deep_sleep_seconds: Some(deep_sleep_seconds),
        }
    }
}

#[cfg(target_os = "espidf")]
pub mod espidf {
    use core::time::Duration;

    use esp_idf_hal::reset::WakeupReason;
    use esp_idf_hal::sleep::DeepSleep;

    pub const SELF_TEST_TIMER_WAKE_SECONDS: u64 = 20;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum WakeProbe {
        Timer,
        Button,
        Ulp,
        Unknown,
        Other(u32),
    }

    impl WakeProbe {
        pub const fn label(self) -> &'static str {
            match self {
                Self::Timer => "timer",
                Self::Button => "button",
                Self::Ulp => "ulp",
                Self::Unknown => "unknown",
                Self::Other(_) => "other",
            }
        }
    }

    impl From<WakeupReason> for WakeProbe {
        fn from(value: WakeupReason) -> Self {
            match value {
                WakeupReason::Timer => Self::Timer,
                WakeupReason::Button => Self::Button,
                WakeupReason::ULP => Self::Ulp,
                WakeupReason::Unknown => Self::Unknown,
                WakeupReason::Other(value) => Self::Other(value),
            }
        }
    }

    pub fn wake_probe() -> WakeProbe {
        WakeupReason::get().into()
    }

    pub fn enter_timer_deep_sleep(seconds: u64) -> Result<(), esp_idf_sys::EspError> {
        let sleep = DeepSleep::new()?.wakeup_on_timer(Duration::from_secs(seconds))?;
        sleep.enter()
    }
}
