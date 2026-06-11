#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChargeState {
    Unknown,
    Discharging,
    Charging,
    Full,
}

pub const BATTERY_SYNC_INTERVAL_SECONDS: u64 = 24 * 60 * 60;
pub const CHARGING_SYNC_INTERVAL_SECONDS: u64 = 60 * 60;

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

    pub const fn externally_powered(&self) -> bool {
        matches!(self.charge_state, ChargeState::Charging | ChargeState::Full)
    }

    pub const fn effective_low_battery(&self) -> bool {
        self.low_battery && !self.externally_powered()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PowerProfile {
    Battery,
    Charging,
    ExternalFull,
    LowBattery,
}

impl PowerProfile {
    pub const fn wake_interval_seconds(self) -> u64 {
        match self {
            Self::Battery | Self::LowBattery => BATTERY_SYNC_INTERVAL_SECONDS,
            Self::Charging | Self::ExternalFull => CHARGING_SYNC_INTERVAL_SECONDS,
        }
    }

    pub const fn cloud_sync_enabled(self) -> bool {
        !matches!(self, Self::LowBattery)
    }
}

impl From<&BatteryStatus> for PowerProfile {
    fn from(status: &BatteryStatus) -> Self {
        if status.effective_low_battery() {
            return Self::LowBattery;
        }

        match status.charge_state {
            ChargeState::Charging => Self::Charging,
            ChargeState::Full => Self::ExternalFull,
            ChargeState::Unknown | ChargeState::Discharging => Self::Battery,
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
            minimum_percent: None,
            minimum_millivolts: None,
        }
    }
}

impl LowBatteryPolicy {
    pub fn is_low_battery(&self, battery: &BatteryStatus) -> bool {
        if battery.externally_powered() {
            return false;
        }

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

    pub const fn wake_at_after(next_wakeup_epoch_seconds: u64, deep_sleep_seconds: u32) -> Self {
        Self {
            next_wakeup_epoch_seconds: Some(next_wakeup_epoch_seconds),
            deep_sleep_seconds: Some(deep_sleep_seconds),
        }
    }
}

pub fn next_wakeup_sleep_plan(
    now_epoch_seconds: u64,
    next_sync_epoch_seconds: u64,
    next_plan_date_change_epoch_seconds: Option<u64>,
    carousel_seconds: Option<u32>,
) -> SleepPlan {
    let carousel_epoch_seconds =
        carousel_seconds.map(|seconds| now_epoch_seconds.saturating_add(u64::from(seconds)));

    let earliest_candidate_epoch_seconds = [
        Some(next_sync_epoch_seconds),
        next_plan_date_change_epoch_seconds,
        carousel_epoch_seconds,
    ]
    .into_iter()
    .flatten()
    .min()
    .unwrap_or(now_epoch_seconds);
    let next_wakeup_epoch_seconds = earliest_candidate_epoch_seconds.max(now_epoch_seconds);

    let deep_sleep_seconds = next_wakeup_epoch_seconds
        .saturating_sub(now_epoch_seconds)
        .min(u64::from(u32::MAX)) as u32;

    SleepPlan::wake_at_after(next_wakeup_epoch_seconds, deep_sleep_seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_sleep_plan_uses_wake_interval_when_it_is_earliest() {
        assert_eq!(
            next_wakeup_sleep_plan(1_000, 1_100, Some(1_200), Some(300)),
            SleepPlan::wake_at_after(1_100, 100)
        );
    }

    #[test]
    fn power_sleep_plan_uses_plan_date_change_when_it_is_earliest() {
        assert_eq!(
            next_wakeup_sleep_plan(1_000, 1_300, Some(1_120), Some(200)),
            SleepPlan::wake_at_after(1_120, 120)
        );
    }

    #[test]
    fn power_sleep_plan_uses_carousel_interval_when_it_is_earliest() {
        assert_eq!(
            next_wakeup_sleep_plan(1_000, 1_300, Some(1_250), Some(60)),
            SleepPlan::wake_at_after(1_060, 60)
        );
    }

    #[test]
    fn power_sleep_plan_wakes_immediately_for_past_wake_time() {
        assert_eq!(
            next_wakeup_sleep_plan(1_000, 900, Some(1_250), Some(60)),
            SleepPlan::wake_at_after(1_000, 0)
        );
    }

    #[test]
    fn power_profile_follows_battery_charge_state() {
        assert_eq!(
            PowerProfile::from(&BatteryStatus::new(
                3_900,
                Some(50),
                ChargeState::Discharging,
                false
            )),
            PowerProfile::Battery
        );
        assert_eq!(
            PowerProfile::from(&BatteryStatus::new(
                4_000,
                Some(70),
                ChargeState::Charging,
                false
            )),
            PowerProfile::Charging
        );
        assert_eq!(
            PowerProfile::from(&BatteryStatus::new(
                4_200,
                Some(100),
                ChargeState::Full,
                false
            )),
            PowerProfile::ExternalFull
        );
        assert_eq!(
            PowerProfile::from(&BatteryStatus::new(
                3_500,
                Some(10),
                ChargeState::Charging,
                true
            )),
            PowerProfile::Charging
        );
    }

    #[test]
    fn externally_powered_status_is_not_effective_low_battery() {
        for charge_state in [ChargeState::Charging, ChargeState::Full] {
            let battery = BatteryStatus::new(0, Some(0), charge_state, true);

            assert!(battery.externally_powered());
            assert!(!battery.effective_low_battery());
            assert!(!LowBatteryPolicy::default().is_low_battery(&battery));
            assert_ne!(PowerProfile::from(&battery), PowerProfile::LowBattery);
        }
    }

    #[test]
    fn discharging_status_keeps_low_battery_policy() {
        let explicit_low = BatteryStatus::new(0, Some(80), ChargeState::Discharging, true);

        assert!(explicit_low.effective_low_battery());
        assert!(LowBatteryPolicy::default().is_low_battery(&explicit_low));
        assert_eq!(PowerProfile::from(&explicit_low), PowerProfile::LowBattery);
    }

    #[test]
    fn default_low_battery_policy_ignores_uncalibrated_percent() {
        let battery = BatteryStatus::new(0, Some(0), ChargeState::Unknown, false);

        assert!(!LowBatteryPolicy::default().is_low_battery(&battery));
    }

    #[test]
    fn percent_threshold_only_applies_when_explicitly_configured() {
        let policy = LowBatteryPolicy {
            minimum_percent: Some(15),
            ..LowBatteryPolicy::default()
        };
        let battery = BatteryStatus::new(0, Some(0), ChargeState::Discharging, false);

        assert!(policy.is_low_battery(&battery));
    }

    #[test]
    fn power_profile_wake_interval_matches_power_state() {
        assert_eq!(
            PowerProfile::Battery.wake_interval_seconds(),
            BATTERY_SYNC_INTERVAL_SECONDS
        );
        assert_eq!(
            PowerProfile::Charging.wake_interval_seconds(),
            CHARGING_SYNC_INTERVAL_SECONDS
        );
        assert_eq!(
            PowerProfile::ExternalFull.wake_interval_seconds(),
            CHARGING_SYNC_INTERVAL_SECONDS
        );
        assert_eq!(
            PowerProfile::LowBattery.wake_interval_seconds(),
            BATTERY_SYNC_INTERVAL_SECONDS
        );
    }

    #[test]
    fn charging_profile_wakes_every_hour() {
        assert_eq!(
            PowerProfile::Charging.wake_interval_seconds(),
            CHARGING_SYNC_INTERVAL_SECONDS
        );
    }

    #[test]
    fn low_battery_profile_skips_cloud_sync() {
        assert!(!PowerProfile::LowBattery.cloud_sync_enabled());
    }

    #[test]
    fn external_full_profile_uses_charging_interval() {
        assert_eq!(
            PowerProfile::ExternalFull.wake_interval_seconds(),
            CHARGING_SYNC_INTERVAL_SECONDS
        );
    }
}

#[cfg(target_os = "espidf")]
pub mod espidf {
    use core::time::Duration;

    use esp_idf_hal::reset::WakeupReason;
    use esp_idf_hal::sleep::DeepSleep;
    use esp_idf_sys::{
        esp, gpio_config, gpio_config_t, gpio_get_level, gpio_int_type_t_GPIO_INTR_DISABLE,
        gpio_mode_t_GPIO_MODE_INPUT, gpio_num_t_GPIO_NUM_4, gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
        gpio_pullup_t_GPIO_PULLUP_ENABLE,
    };

    pub const SELF_TEST_TIMER_WAKE_SECONDS: u64 = 20;
    pub const SELF_TEST_KEY_GPIO: i32 = gpio_num_t_GPIO_NUM_4;
    const SELF_TEST_KEY_HOLD_MS: u32 = 1_800;
    const SELF_TEST_KEY_SAMPLE_MS: u32 = 30;

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

    pub fn self_test_key_long_pressed() -> bool {
        if configure_self_test_key().is_err() {
            return false;
        }

        if !self_test_key_pressed() {
            return false;
        }

        let samples = SELF_TEST_KEY_HOLD_MS / SELF_TEST_KEY_SAMPLE_MS;
        for _ in 0..samples {
            esp_idf_hal::delay::FreeRtos::delay_ms(SELF_TEST_KEY_SAMPLE_MS);
            if !self_test_key_pressed() {
                return false;
            }
        }

        true
    }

    fn configure_self_test_key() -> Result<(), esp_idf_sys::EspError> {
        let config = gpio_config_t {
            pin_bit_mask: 1u64 << SELF_TEST_KEY_GPIO,
            mode: gpio_mode_t_GPIO_MODE_INPUT,
            pull_up_en: gpio_pullup_t_GPIO_PULLUP_ENABLE,
            pull_down_en: gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
            intr_type: gpio_int_type_t_GPIO_INTR_DISABLE,
        };

        unsafe { esp!(gpio_config(&config)) }
    }

    fn self_test_key_pressed() -> bool {
        unsafe { gpio_get_level(SELF_TEST_KEY_GPIO) == 0 }
    }

    pub fn enter_timer_deep_sleep(seconds: u64) -> Result<(), esp_idf_sys::EspError> {
        let sleep = DeepSleep::new()?.wakeup_on_timer(Duration::from_secs(seconds))?;
        sleep.enter()
    }
}
