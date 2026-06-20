#[cfg(target_os = "espidf")]
pub mod espidf {
    use esp_idf_svc::nvs::{EspDefaultNvsPartition, EspNvs};
    use esp_idf_sys::EspError;

    const NAMESPACE: &str = "epaper";
    const SELF_TEST_REQUEST_KEY: &str = "selftest";

    pub fn self_test_requested() -> Result<bool, EspError> {
        let partition = EspDefaultNvsPartition::take()?;
        let nvs = EspNvs::new(partition, NAMESPACE, false)?;
        Ok(nvs.get_u8(SELF_TEST_REQUEST_KEY)?.unwrap_or(0) != 0)
    }

    pub fn request_self_test() -> Result<(), EspError> {
        let partition = EspDefaultNvsPartition::take()?;
        let nvs = EspNvs::new(partition, NAMESPACE, true)?;
        nvs.set_u8(SELF_TEST_REQUEST_KEY, 1)
    }

    pub fn clear_self_test_request() -> Result<(), EspError> {
        let partition = EspDefaultNvsPartition::take()?;
        let nvs = EspNvs::new(partition, NAMESPACE, true)?;
        nvs.remove(SELF_TEST_REQUEST_KEY)?;
        Ok(())
    }
}
