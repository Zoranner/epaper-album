#[cfg(target_os = "espidf")]
pub mod espidf {
    use esp_idf_hal::i2c::{I2cConfig, I2cDriver};
    use esp_idf_hal::units::FromValueType;
    use esp_idf_sys::TickType_t;

    const AXP2101_ADDRESS: u8 = 0x34;
    const AXP2101_CHIP_ID: u8 = 0x4A;
    const REG_CHIP_ID: u8 = 0x03;
    const REG_DC_ONOFF: u8 = 0x80;
    const REG_DC1_VOLTAGE: u8 = 0x82;
    const REG_LDO_ONOFF0: u8 = 0x90;
    const REG_ALDO1_VOLTAGE: u8 = 0x92;
    const REG_ALDO2_VOLTAGE: u8 = 0x93;
    const REG_ALDO3_VOLTAGE: u8 = 0x94;
    const REG_ALDO4_VOLTAGE: u8 = 0x95;
    const DC1_3300MV: u8 = ((3300u16 - 1500) / 100) as u8;
    const ALDO_3300MV: u8 = ((3300u16 - 500) / 100) as u8;
    const I2C_TIMEOUT_TICKS: TickType_t = 1000;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct PmicProbe {
        pub chip_id: u8,
        pub dc_onoff: u8,
        pub ldo_onoff0: u8,
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

        Ok(PmicProbe {
            chip_id,
            dc_onoff: read_register(&mut i2c, REG_DC_ONOFF)?,
            ldo_onoff0: read_register(&mut i2c, REG_LDO_ONOFF0)?,
        })
    }

    pub fn chip_id_is_axp2101(probe: PmicProbe) -> bool {
        probe.chip_id == AXP2101_CHIP_ID
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
}
