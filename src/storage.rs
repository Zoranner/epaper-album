use std::path::{Path, PathBuf};

pub const ALBUM_ROOT: &str = "/sdcard/epaper-album";
pub const PLANS_CURRENT_PATH: &str = "/sdcard/epaper-album/plans/current.json";
pub const CACHE_INDEX_PATH: &str = "/sdcard/epaper-album/cache-index.json";
pub const DISPLAY_STATE_PATH: &str = "/sdcard/epaper-album/display-state.json";
pub const IMAGES_DIR: &str = "/sdcard/epaper-album/images";
pub const SPRITES_DIR: &str = "/sdcard/epaper-album/sprites";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageRead {
    Text(String),
    Missing,
    MountError,
    ReadError,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageBinaryRead {
    Bytes(Vec<u8>),
    Missing,
    FormatError,
    MountError,
    ReadError,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageWrite {
    Written,
    MountError,
    WriteError,
}

pub fn image_bmp_path(sha256: &str) -> PathBuf {
    Path::new(IMAGES_DIR).join(format!("{sha256}.bmp"))
}

pub fn sprite_bmp_path(key: &str) -> PathBuf {
    Path::new(SPRITES_DIR).join(format!("{key}.bmp"))
}

#[cfg(not(target_os = "espidf"))]
pub fn read_text_file(path: impl AsRef<Path>) -> StorageRead {
    read_host_text_file(path.as_ref())
}

#[cfg(not(target_os = "espidf"))]
fn read_host_text_file(path: &Path) -> StorageRead {
    match std::fs::read_to_string(path) {
        Ok(content) => StorageRead::Text(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => StorageRead::Missing,
        Err(_) => StorageRead::ReadError,
    }
}

#[cfg(not(target_os = "espidf"))]
pub fn read_binary_file(path: impl AsRef<Path>) -> StorageBinaryRead {
    read_host_binary_file(path.as_ref())
}

#[cfg(not(target_os = "espidf"))]
fn read_host_binary_file(path: &Path) -> StorageBinaryRead {
    match std::fs::read(path) {
        Ok(content) => StorageBinaryRead::Bytes(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => StorageBinaryRead::Missing,
        Err(_) => StorageBinaryRead::ReadError,
    }
}

#[cfg(not(target_os = "espidf"))]
pub fn write_text_file_atomic(path: impl AsRef<Path>, content: &str) -> StorageWrite {
    write_host_file_atomic(path.as_ref(), content.as_bytes())
}

#[cfg(not(target_os = "espidf"))]
pub fn write_binary_file_atomic(path: impl AsRef<Path>, content: &[u8]) -> StorageWrite {
    write_host_file_atomic(path.as_ref(), content)
}

#[cfg(not(target_os = "espidf"))]
fn write_host_file_atomic(path: &Path, content: &[u8]) -> StorageWrite {
    match write_file_atomic(path, content) {
        Ok(()) => StorageWrite::Written,
        Err(_) => StorageWrite::WriteError,
    }
}

#[cfg(target_os = "espidf")]
pub fn read_text_file(path: impl AsRef<Path>) -> StorageRead {
    read_espidf_text_file(path.as_ref())
}

#[cfg(target_os = "espidf")]
fn read_espidf_text_file(path: &Path) -> StorageRead {
    match with_mounted_sdcard(|| std::fs::read_to_string(path)) {
        Ok(Ok(content)) => StorageRead::Text(content),
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => StorageRead::Missing,
        Ok(Err(_)) => StorageRead::ReadError,
        Err(_) => StorageRead::MountError,
    }
}

#[cfg(target_os = "espidf")]
pub fn read_binary_file(path: impl AsRef<Path>) -> StorageBinaryRead {
    read_espidf_binary_file(path.as_ref())
}

#[cfg(target_os = "espidf")]
fn read_espidf_binary_file(path: &Path) -> StorageBinaryRead {
    match with_mounted_sdcard(|| std::fs::read(path)) {
        Ok(Ok(content)) => StorageBinaryRead::Bytes(content),
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            StorageBinaryRead::Missing
        }
        Ok(Err(_)) => StorageBinaryRead::ReadError,
        Err(_) => StorageBinaryRead::MountError,
    }
}

#[cfg(target_os = "espidf")]
pub fn write_text_file_atomic(path: impl AsRef<Path>, content: &str) -> StorageWrite {
    write_espidf_file_atomic(path.as_ref(), content.as_bytes())
}

#[cfg(target_os = "espidf")]
pub fn write_binary_file_atomic(path: impl AsRef<Path>, content: &[u8]) -> StorageWrite {
    write_espidf_file_atomic(path.as_ref(), content)
}

#[cfg(target_os = "espidf")]
fn write_espidf_file_atomic(path: &Path, content: &[u8]) -> StorageWrite {
    match with_mounted_sdcard(|| write_file_atomic(path, content)) {
        Ok(Ok(())) => StorageWrite::Written,
        Ok(Err(_)) => StorageWrite::WriteError,
        Err(_) => StorageWrite::MountError,
    }
}

fn write_file_atomic(path: &Path, content: &[u8]) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let temp_path = temp_path_for(path);
    std::fs::write(&temp_path, content)?;

    #[cfg(windows)]
    if path.exists() {
        std::fs::remove_file(path)?;
    }

    match std::fs::rename(&temp_path, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = std::fs::remove_file(&temp_path);
            Err(error)
        }
    }
}

fn temp_path_for(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("epaper-album");
    path.with_file_name(format!("{file_name}.tmp"))
}

#[cfg(target_os = "espidf")]
fn with_mounted_sdcard<T>(
    operation: impl FnOnce() -> Result<T, std::io::Error>,
) -> Result<Result<T, std::io::Error>, esp_idf_sys::EspError> {
    use esp_idf_svc::hal::peripherals::Peripherals;

    let peripherals = Peripherals::take()?;
    let pins = peripherals.pins;

    with_mounted_sdcard_parts(
        peripherals.sdmmc1,
        pins.gpio41,
        pins.gpio39,
        pins.gpio40,
        pins.gpio1,
        pins.gpio2,
        pins.gpio38,
        operation,
    )
}

#[cfg(target_os = "espidf")]
#[allow(clippy::too_many_arguments)]
pub fn read_espidf_text_file_with_sdmmc(
    path: &Path,
    sdmmc: esp_idf_svc::hal::sd::mmc::SDMMC1<'static>,
    cmd: esp_idf_svc::hal::gpio::Gpio41<'static>,
    clk: esp_idf_svc::hal::gpio::Gpio39<'static>,
    d0: esp_idf_svc::hal::gpio::Gpio40<'static>,
    d1: esp_idf_svc::hal::gpio::Gpio1<'static>,
    d2: esp_idf_svc::hal::gpio::Gpio2<'static>,
    d3: esp_idf_svc::hal::gpio::Gpio38<'static>,
) -> StorageRead {
    match with_mounted_sdcard_parts(sdmmc, cmd, clk, d0, d1, d2, d3, || {
        std::fs::read_to_string(path)
    }) {
        Ok(Ok(content)) => StorageRead::Text(content),
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => StorageRead::Missing,
        Ok(Err(_)) => StorageRead::ReadError,
        Err(_) => StorageRead::MountError,
    }
}

#[cfg(target_os = "espidf")]
#[allow(clippy::too_many_arguments)]
pub fn with_mounted_sdcard_parts<T>(
    sdmmc: esp_idf_svc::hal::sd::mmc::SDMMC1<'static>,
    cmd: esp_idf_svc::hal::gpio::Gpio41<'static>,
    clk: esp_idf_svc::hal::gpio::Gpio39<'static>,
    d0: esp_idf_svc::hal::gpio::Gpio40<'static>,
    d1: esp_idf_svc::hal::gpio::Gpio1<'static>,
    d2: esp_idf_svc::hal::gpio::Gpio2<'static>,
    d3: esp_idf_svc::hal::gpio::Gpio38<'static>,
    operation: impl FnOnce() -> Result<T, std::io::Error>,
) -> Result<Result<T, std::io::Error>, esp_idf_sys::EspError> {
    use esp_idf_svc::fs::fatfs::Fatfs;
    use esp_idf_svc::hal::sd::{
        mmc::SdMmcHostConfiguration, mmc::SdMmcHostDriver, SdCardConfiguration, SdCardDriver,
    };
    use esp_idf_svc::io::vfs::MountedFatfs;

    let sd_card_driver = SdCardDriver::new_mmc(
        SdMmcHostDriver::new_4bits(
            sdmmc,
            cmd,
            clk,
            d0,
            d1,
            d2,
            d3,
            None::<esp_idf_svc::hal::gpio::AnyIOPin>,
            None::<esp_idf_svc::hal::gpio::AnyIOPin>,
            &SdMmcHostConfiguration::new(),
        )?,
        &SdCardConfiguration::new(),
    )?;
    let fatfs = Fatfs::new_sdcard(0, sd_card_driver)?;
    let _mounted_fatfs = MountedFatfs::mount(fatfs, "/sdcard", 4)?;

    Ok(operation())
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

    #[test]
    fn builds_album_resource_paths() {
        assert_eq!(
            Path::new(PLANS_CURRENT_PATH),
            Path::new(ALBUM_ROOT).join("plans").join("current.json")
        );
        assert_eq!(
            image_bmp_path("abc"),
            Path::new(ALBUM_ROOT).join("images").join("abc.bmp")
        );
        assert_eq!(
            sprite_bmp_path("battery-low"),
            Path::new(ALBUM_ROOT)
                .join("sprites")
                .join("battery-low.bmp")
        );
    }

    #[test]
    fn writes_text_file_atomically_on_host() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("plans").join("current.json");

        let result = write_text_file_atomic(&file_path, r#"{"content_hash":"v1"}"#);

        assert_eq!(result, StorageWrite::Written);
        assert_eq!(
            std::fs::read_to_string(&file_path).unwrap(),
            r#"{"content_hash":"v1"}"#
        );
        assert!(!file_path.with_file_name("current.json.tmp").exists());
    }

    #[test]
    fn writes_binary_file_atomically_on_host() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("images").join("abc.bmp");

        let result = write_binary_file_atomic(&file_path, &[0x42, 0x4d, 0x00]);

        assert_eq!(result, StorageWrite::Written);
        assert_eq!(
            read_binary_file(&file_path),
            StorageBinaryRead::Bytes(vec![0x42, 0x4d, 0x00])
        );
    }
}
