pub mod app_files;

use crate::model::PlanSnapshot;
use std::path::{Path, PathBuf};

pub const DATA_ROOT: &str = "/sdcard/data";
pub const PLAN_PATH: &str = "/sdcard/data/plan.json";
pub const STATE_PATH: &str = "/sdcard/data/state.json";
pub const IMAGES_DIR: &str = "/sdcard/data/images";
pub const SPRITES_DIR: &str = "/sdcard/data/sprites";

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageJsonRead<T> {
    Value(T),
    Missing,
    MountError,
    ReadError,
    ParseError,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageJsonWrite {
    Written,
    SerializeError,
    MountError,
    WriteError,
}

pub trait ResourceStore {
    fn save_plan_snapshot(&mut self, snapshot: &PlanSnapshot) -> StorageJsonWrite;
    fn save_image_bytes(&mut self, sha256: &str, content: &[u8]) -> StorageWrite;
    fn save_sprite_bytes(&mut self, sha256: &str, content: &[u8]) -> StorageWrite;
}

#[derive(Debug, Default)]
pub struct SdCardResourceStore;

#[derive(Debug, Default)]
pub struct MountedSdCardResourceStore;

impl ResourceStore for SdCardResourceStore {
    fn save_plan_snapshot(&mut self, snapshot: &PlanSnapshot) -> StorageJsonWrite {
        write_json_file_atomic(PLAN_PATH, snapshot)
    }

    fn save_image_bytes(&mut self, sha256: &str, content: &[u8]) -> StorageWrite {
        write_binary_file_atomic(image_bmp_path(sha256), content)
    }

    fn save_sprite_bytes(&mut self, sha256: &str, content: &[u8]) -> StorageWrite {
        write_binary_file_atomic(sprite_bmp_path(sha256), content)
    }
}

impl ResourceStore for MountedSdCardResourceStore {
    fn save_plan_snapshot(&mut self, snapshot: &PlanSnapshot) -> StorageJsonWrite {
        write_json_file_atomic_mounted(PLAN_PATH, snapshot)
    }

    fn save_image_bytes(&mut self, sha256: &str, content: &[u8]) -> StorageWrite {
        write_binary_file_atomic_mounted(image_bmp_path(sha256), content)
    }

    fn save_sprite_bytes(&mut self, sha256: &str, content: &[u8]) -> StorageWrite {
        write_binary_file_atomic_mounted(sprite_bmp_path(sha256), content)
    }
}

pub fn image_bmp_path(sha256: &str) -> PathBuf {
    Path::new(IMAGES_DIR).join(format!("{sha256}.bmp"))
}

pub fn sprite_bmp_path(sha256: &str) -> PathBuf {
    Path::new(SPRITES_DIR).join(format!("{sha256}.bmp"))
}

pub fn parse_json_str<T>(content: &str) -> Result<T, serde_json::Error>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_str(content)
}

pub fn to_json_string<T>(value: &T) -> Result<String, serde_json::Error>
where
    T: serde::Serialize,
{
    serde_json::to_string_pretty(value)
}

pub fn read_json_file<T>(path: impl AsRef<Path>) -> StorageJsonRead<T>
where
    T: serde::de::DeserializeOwned,
{
    match read_text_file(path) {
        StorageRead::Text(content) => match parse_json_str(&content) {
            Ok(value) => StorageJsonRead::Value(value),
            Err(_) => StorageJsonRead::ParseError,
        },
        StorageRead::Missing => StorageJsonRead::Missing,
        StorageRead::MountError => StorageJsonRead::MountError,
        StorageRead::ReadError => StorageJsonRead::ReadError,
    }
}

pub fn write_json_file_atomic<T>(path: impl AsRef<Path>, value: &T) -> StorageJsonWrite
where
    T: serde::Serialize,
{
    let content = match to_json_string(value) {
        Ok(content) => content,
        Err(_) => return StorageJsonWrite::SerializeError,
    };

    match write_text_file_atomic(path, &content) {
        StorageWrite::Written => StorageJsonWrite::Written,
        StorageWrite::MountError => StorageJsonWrite::MountError,
        StorageWrite::WriteError => StorageJsonWrite::WriteError,
    }
}

pub fn read_text_file_mounted(path: impl AsRef<Path>) -> StorageRead {
    match std::fs::read_to_string(path) {
        Ok(content) => StorageRead::Text(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => StorageRead::Missing,
        Err(_) => StorageRead::ReadError,
    }
}

pub fn read_binary_file_mounted(path: impl AsRef<Path>) -> StorageBinaryRead {
    match std::fs::read(path) {
        Ok(content) => StorageBinaryRead::Bytes(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => StorageBinaryRead::Missing,
        Err(_) => StorageBinaryRead::ReadError,
    }
}

pub fn read_json_file_mounted<T>(path: impl AsRef<Path>) -> StorageJsonRead<T>
where
    T: serde::de::DeserializeOwned,
{
    match read_text_file_mounted(path) {
        StorageRead::Text(content) => match parse_json_str(&content) {
            Ok(value) => StorageJsonRead::Value(value),
            Err(_) => StorageJsonRead::ParseError,
        },
        StorageRead::Missing => StorageJsonRead::Missing,
        StorageRead::MountError => StorageJsonRead::MountError,
        StorageRead::ReadError => StorageJsonRead::ReadError,
    }
}

pub fn write_text_file_atomic_mounted(path: impl AsRef<Path>, content: &str) -> StorageWrite {
    match write_file_atomic(path.as_ref(), content.as_bytes()) {
        Ok(()) => StorageWrite::Written,
        Err(_) => StorageWrite::WriteError,
    }
}

pub fn write_binary_file_atomic_mounted(path: impl AsRef<Path>, content: &[u8]) -> StorageWrite {
    match write_file_atomic(path.as_ref(), content) {
        Ok(()) => StorageWrite::Written,
        Err(_) => StorageWrite::WriteError,
    }
}

pub fn write_json_file_atomic_mounted<T>(path: impl AsRef<Path>, value: &T) -> StorageJsonWrite
where
    T: serde::Serialize,
{
    let content = match to_json_string(value) {
        Ok(content) => content,
        Err(_) => return StorageJsonWrite::SerializeError,
    };

    match write_text_file_atomic_mounted(path, &content) {
        StorageWrite::Written => StorageJsonWrite::Written,
        StorageWrite::MountError => StorageJsonWrite::MountError,
        StorageWrite::WriteError => StorageJsonWrite::WriteError,
    }
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
        assert_eq!(Path::new(PLAN_PATH), Path::new(DATA_ROOT).join("plan.json"));
        assert_eq!(
            image_bmp_path("abc"),
            Path::new(DATA_ROOT).join("images").join("abc.bmp")
        );
        assert_eq!(
            sprite_bmp_path("abc"),
            Path::new(DATA_ROOT).join("sprites").join("abc.bmp")
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

    #[test]
    fn reads_and_writes_typed_json_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("state.json");
        let state = crate::model::ResourceIndex {
            resources: vec![crate::model::CachedResource {
                sha256: "abc".to_string(),
                byte_size: 128,
                last_used_at_unix_secs: 9,
            }],
        };

        let write_result = write_json_file_atomic(&file_path, &state);
        let read_result = read_json_file(&file_path);

        assert_eq!(write_result, StorageJsonWrite::Written);
        assert_eq!(read_result, StorageJsonRead::Value(state));
    }

    #[test]
    fn reports_typed_json_parse_error() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("broken.json");
        std::fs::write(&file_path, "{").unwrap();

        let read_result: StorageJsonRead<crate::model::ResourceIndex> = read_json_file(&file_path);

        assert_eq!(read_result, StorageJsonRead::ParseError);
    }

    #[test]
    fn mounted_helpers_read_and_write_without_mounting_again() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("mounted").join("state.json");
        let state = crate::model::ResourceIndex {
            resources: vec![crate::model::CachedResource {
                sha256: "abc".to_string(),
                byte_size: 128,
                last_used_at_unix_secs: 9,
            }],
        };

        let write_result = write_json_file_atomic_mounted(&file_path, &state);
        let read_result = read_json_file_mounted(&file_path);

        assert_eq!(write_result, StorageJsonWrite::Written);
        assert_eq!(read_result, StorageJsonRead::Value(state));
    }
}
