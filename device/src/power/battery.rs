use super::schedule::HOURLY_RUN_INTERVAL_SECONDS;

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
    pub external_powered: bool,
    pub low_battery: bool,
}

impl BatteryStatus {
    pub const fn new(
        millivolts: u16,
        percent: Option<u8>,
        charge_state: ChargeState,
        low_battery: bool,
    ) -> Self {
        Self::with_external_power(
            millivolts,
            percent,
            charge_state,
            matches!(charge_state, ChargeState::Charging | ChargeState::Full),
            low_battery,
        )
    }

    pub const fn with_external_power(
        millivolts: u16,
        percent: Option<u8>,
        charge_state: ChargeState,
        external_powered: bool,
        low_battery: bool,
    ) -> Self {
        Self {
            millivolts,
            percent,
            charge_state,
            external_powered,
            low_battery,
        }
    }

    pub const fn unknown() -> Self {
        Self {
            millivolts: 0,
            percent: None,
            charge_state: ChargeState::Unknown,
            external_powered: false,
            low_battery: false,
        }
    }

    pub const fn externally_powered(&self) -> bool {
        self.external_powered
    }

    pub const fn effective_low_battery(&self) -> bool {
        self.low_battery && !self.externally_powered()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PowerProfile {
    Battery,
    External,
    LowBattery,
}

impl PowerProfile {
    pub const fn run_interval_seconds(self) -> u64 {
        match self {
            Self::Battery | Self::External | Self::LowBattery => HOURLY_RUN_INTERVAL_SECONDS,
        }
    }
}

impl From<&BatteryStatus> for PowerProfile {
    fn from(status: &BatteryStatus) -> Self {
        if status.effective_low_battery() {
            return Self::LowBattery;
        }

        if status.externally_powered() {
            Self::External
        } else {
            Self::Battery
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LowBatteryPolicy {
    pub stop_cloud_sync: bool,
    pub minimum_percent: Option<u8>,
    pub minimum_millivolts: Option<u16>,
}

impl Default for LowBatteryPolicy {
    fn default() -> Self {
        Self {
            stop_cloud_sync: true,
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

#[cfg(test)]
mod tests {
    use super::*;

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
            PowerProfile::External
        );
        assert_eq!(
            PowerProfile::from(&BatteryStatus::new(
                4_200,
                Some(100),
                ChargeState::Full,
                false
            )),
            PowerProfile::External
        );
        assert_eq!(
            PowerProfile::from(&BatteryStatus::new(
                3_500,
                Some(10),
                ChargeState::Charging,
                true
            )),
            PowerProfile::External
        );
    }

    #[test]
    fn full_battery_without_external_input_stays_in_battery_profile() {
        let battery =
            BatteryStatus::with_external_power(0, Some(100), ChargeState::Full, false, false);

        assert!(!battery.externally_powered());
        assert_eq!(PowerProfile::from(&battery), PowerProfile::Battery);
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
    fn power_profile_run_interval_matches_power_state() {
        assert_eq!(
            PowerProfile::Battery.run_interval_seconds(),
            HOURLY_RUN_INTERVAL_SECONDS
        );
        assert_eq!(
            PowerProfile::External.run_interval_seconds(),
            HOURLY_RUN_INTERVAL_SECONDS
        );
        assert_eq!(
            PowerProfile::LowBattery.run_interval_seconds(),
            HOURLY_RUN_INTERVAL_SECONDS
        );
    }

    #[test]
    fn external_profile_wakes_every_hour() {
        assert_eq!(
            PowerProfile::External.run_interval_seconds(),
            HOURLY_RUN_INTERVAL_SECONDS
        );
    }

    #[test]
    fn external_profile_uses_hourly_interval() {
        assert_eq!(
            PowerProfile::External.run_interval_seconds(),
            HOURLY_RUN_INTERVAL_SECONDS
        );
    }
}
