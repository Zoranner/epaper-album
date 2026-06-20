mod es8311;
mod i2s;

use crate::platform::audio::self_test_tone_pcm;
use es8311::{enable_pa, init_es8311, I2cBusGuard};
use esp_idf_sys::EspError;
use i2s::I2sTxGuard;

pub fn play_self_test_request_tone() -> Result<(), EspError> {
    let _i2c = I2cBusGuard::install()?;
    init_es8311()?;
    let mut i2s = I2sTxGuard::install()?;
    enable_pa(true)?;

    let pcm = self_test_tone_pcm();
    i2s.write_all(&pcm)?;
    esp_idf_hal::delay::FreeRtos::delay_ms(30);

    enable_pa(false)?;
    Ok(())
}
