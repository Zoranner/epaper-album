#[cfg(target_os = "espidf")]
pub mod espidf {
    use crate::power::{BatteryStatus, ChargeState};
    use esp_idf_hal::i2c::{I2cConfig, I2cDriver};
    use esp_idf_hal::units::FromValueType;
    use esp_idf_sys::TickType_t;

    const AXP2101_ADDRESS: u8 = 0x34;
    const AXP2101_CHIP_ID: u8 = 0x4A;
    const REG_STATUS1: u8 = 0x00;
    const REG_STATUS2: u8 = 0x01;
    const REG_CHIP_ID: u8 = 0x03;
    const REG_DC_ONOFF: u8 = 0x80;
    const REG_DC1_VOLTAGE: u8 = 0x82;
    const REG_LDO_ONOFF0: u8 = 0x90;
    const REG_ALDO1_VOLTAGE: u8 = 0x92;
    const REG_ALDO2_VOLTAGE: u8 = 0x93;
    const REG_ALDO3_VOLTAGE: u8 = 0x94;
    const REG_ALDO4_VOLTAGE: u8 = 0x95;
    const REG_BAT_PERCENT_DATA: u8 = 0xA4;
    const DC1_3300MV: u8 = ((3300u16 - 1500) / 100) as u8;
    const ALDO_3300MV: u8 = ((3300u16 - 500) / 100) as u8;
    const I2C_TIMEOUT_TICKS: TickType_t = 1000;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct PmicProbe {
        pub chip_id: u8,
        pub status1: u8,
        pub status2: u8,
        pub dc_onoff: u8,
        pub ldo_onoff0: u8,
        pub battery: BatteryStatus,
    }

    pub fn init_axp2101_for_photo_painter(
        i2c0: esp_idf_hal::i2c::I2C0<'static>,
        sda: esp_idf_hal::gpio::Gpio47<'static>,
        scl: esp_idf_hal::gpio::Gpio48<'static>,
    ) -> Result<PmicProbe, esp_idf_sys::EspError> {
        let config = I2cConfig::new().baudrate(100.kHz().into());
        let mut i2c = I2cDriver::new(i2c0, sda, scl, &config)?;
        let chip_id = read_register(&mut i2c, REG_CHIP_ID)?;

        write_register(&mut i2c, REG_DC1_VOLTAGE, DC1_3300MV)?;
        write_register(&mut i2c, REG_ALDO1_VOLTAGE, ALDO_3300MV)?;
        write_register(&mut i2c, REG_ALDO2_VOLTAGE, ALDO_3300MV)?;
        write_register(&mut i2c, REG_ALDO3_VOLTAGE, ALDO_3300MV)?;
        write_register(&mut i2c, REG_ALDO4_VOLTAGE, ALDO_3300MV)?;

        let dc_onoff = read_register(&mut i2c, REG_DC_ONOFF)? | 0x01;
        write_register(&mut i2c, REG_DC_ONOFF, dc_onoff)?;

        let ldo_onoff0 = read_register(&mut i2c, REG_LDO_ONOFF0)? | 0x0F;
        write_register(&mut i2c, REG_LDO_ONOFF0, ldo_onoff0)?;

        let (battery, status1, status2) = read_battery_status(&mut i2c)?;

        Ok(PmicProbe {
            chip_id,
            status1,
            status2,
            dc_onoff: read_register(&mut i2c, REG_DC_ONOFF)?,
            ldo_onoff0: read_register(&mut i2c, REG_LDO_ONOFF0)?,
            battery,
        })
    }

    pub fn chip_id_is_axp2101(probe: PmicProbe) -> bool {
        probe.chip_id == AXP2101_CHIP_ID
    }

    fn read_battery_status(
        i2c: &mut I2cDriver<'_>,
    ) -> Result<(BatteryStatus, u8, u8), esp_idf_sys::EspError> {
        let status1 = read_register(i2c, REG_STATUS1)?;
        let status2 = read_register(i2c, REG_STATUS2)?;
        let percent = read_battery_percent(i2c)?;
        let charge_state = charge_state_from_status(status1, status2);

        Ok((
            BatteryStatus::new(0, percent, charge_state, false),
            status1,
            status2,
        ))
    }

    pub fn status_summary(status1: u8, status2: u8) -> PmicStatusSummary {
        PmicStatusSummary {
            battery_connected: status1 & (1 << 3) != 0,
            vbus_good: status1 & (1 << 5) != 0,
            battery_current_direction: (status2 >> 5) & 0x03,
            charge_step: status2 & 0x07,
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct PmicStatusSummary {
        pub battery_connected: bool,
        pub vbus_good: bool,
        pub battery_current_direction: u8,
        pub charge_step: u8,
    }

    fn read_battery_percent(i2c: &mut I2cDriver<'_>) -> Result<Option<u8>, esp_idf_sys::EspError> {
        let raw = read_register(i2c, REG_BAT_PERCENT_DATA)?;
        Ok((raw <= 100).then_some(raw))
    }

    fn charge_state_from_status(status1: u8, status2: u8) -> ChargeState {
        let summary = status_summary(status1, status2);

        match (
            summary.battery_current_direction,
            summary.charge_step,
            summary.vbus_good,
            summary.battery_connected,
        ) {
            (0x01, _, _, true) => ChargeState::Charging,
            (_, 0x04, true, true) => ChargeState::Full,
            (0x02, _, _, true) => ChargeState::Discharging,
            (0x00, _, true, _) => ChargeState::Full,
            (_, _, true, false) => ChargeState::Full,
            _ => ChargeState::Unknown,
        }
    }

    fn read_register(i2c: &mut I2cDriver<'_>, register: u8) -> Result<u8, esp_idf_sys::EspError> {
        let mut value = [0u8; 1];
        i2c.write_read(AXP2101_ADDRESS, &[register], &mut value, I2C_TIMEOUT_TICKS)?;
        Ok(value[0])
    }

    fn write_register(
        i2c: &mut I2cDriver<'_>,
        register: u8,
        value: u8,
    ) -> Result<(), esp_idf_sys::EspError> {
        i2c.write(AXP2101_ADDRESS, &[register, value], I2C_TIMEOUT_TICKS)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn maps_axp2101_status_to_charge_state() {
            let battery_connected = 1 << 3;
            let vbus_good = 1 << 5;

            assert_eq!(
                charge_state_from_status(battery_connected | vbus_good, 0x01 << 5),
                ChargeState::Charging
            );
            assert_eq!(
                charge_state_from_status(battery_connected | vbus_good, 0x04),
                ChargeState::Full
            );
            assert_eq!(
                charge_state_from_status(battery_connected, 0x02 << 5),
                ChargeState::Discharging
            );
            assert_eq!(charge_state_from_status(vbus_good, 0x00), ChargeState::Full);
        }
    }
}
