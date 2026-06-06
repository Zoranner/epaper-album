#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageRead {
    Text(String),
    Missing,
    MountError,
    ReadError,
}

#[cfg(not(target_os = "espidf"))]
pub fn read_text_file(path: impl AsRef<std::path::Path>) -> StorageRead {
    read_host_text_file(path.as_ref())
}

#[cfg(not(target_os = "espidf"))]
fn read_host_text_file(path: &std::path::Path) -> StorageRead {
    match std::fs::read_to_string(path) {
        Ok(content) => StorageRead::Text(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => StorageRead::Missing,
        Err(_) => StorageRead::ReadError,
    }
}

#[cfg(target_os = "espidf")]
pub fn read_text_file(path: impl AsRef<std::path::Path>) -> StorageRead {
    read_espidf_text_file(path.as_ref())
}

#[cfg(target_os = "espidf")]
fn read_espidf_text_file(path: &std::path::Path) -> StorageRead {
    match with_mounted_sdcard(|| std::fs::read_to_string(path)) {
        Ok(Ok(content)) => StorageRead::Text(content),
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => StorageRead::Missing,
        Ok(Err(_)) => StorageRead::ReadError,
        Err(_) => StorageRead::MountError,
    }
}

#[cfg(target_os = "espidf")]
fn with_mounted_sdcard<T>(
    read: impl FnOnce() -> Result<T, std::io::Error>,
) -> Result<Result<T, std::io::Error>, esp_idf_sys::EspError> {
    use esp_idf_svc::fs::fatfs::Fatfs;
    use esp_idf_svc::hal::peripherals::Peripherals;
    use esp_idf_svc::hal::sd::{
        mmc::SdMmcHostConfiguration, mmc::SdMmcHostDriver, SdCardConfiguration, SdCardDriver,
    };
    use esp_idf_svc::io::vfs::MountedFatfs;

    let peripherals = Peripherals::take()?;
    let pins = peripherals.pins;

    let sd_card_driver = SdCardDriver::new_mmc(
        SdMmcHostDriver::new_4bits(
            peripherals.sdmmc1,
            pins.gpio41,
            pins.gpio39,
            pins.gpio40,
            pins.gpio1,
            pins.gpio2,
            pins.gpio38,
            None::<esp_idf_svc::hal::gpio::AnyIOPin>,
            None::<esp_idf_svc::hal::gpio::AnyIOPin>,
            &SdMmcHostConfiguration::new(),
        )?,
        &SdCardConfiguration::new(),
    )?;
    let fatfs = Fatfs::new_sdcard(0, sd_card_driver)?;
    let _mounted_fatfs = MountedFatfs::mount(fatfs, "/sdcard", 4)?;

    Ok(read())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_existing_text_file_on_host() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("config.toml");
        std::fs::write(&file_path, "wifi_ssid = \"Home\"").unwrap();

        let result = read_text_file(&file_path);

        assert_eq!(
            result,
            StorageRead::Text("wifi_ssid = \"Home\"".to_string())
        );
    }

    #[test]
    fn reports_missing_file_on_host_as_available_storage() {
        let temp_dir = tempfile::tempdir().unwrap();

        let result = read_text_file(temp_dir.path().join("missing.toml"));

        assert_eq!(result, StorageRead::Missing);
    }
}
