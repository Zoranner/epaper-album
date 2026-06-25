use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortEntry {
    pub name: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlashCommand {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlashError {
    MissingEspflash,
    MissingPort,
    MissingImage,
    InvalidImagePath(String),
    InvalidImageExtension,
    ImageNotFound(PathBuf),
    ImageNotFile(PathBuf),
    FlashFailed(String),
}

impl fmt::Display for FlashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingEspflash => write!(f, "未找到 espflash，请确认已加入 PATH"),
            Self::MissingPort => write!(f, "未选择串口"),
            Self::MissingImage => write!(f, "未选择固件文件"),
            Self::InvalidImagePath(path) => write!(f, "固件路径无效: {path}"),
            Self::InvalidImageExtension => write!(f, "固件文件必须是 *-esp32s3-merged.bin"),
            Self::ImageNotFound(path) => write!(f, "固件文件不存在: {}", path.display()),
            Self::ImageNotFile(path) => write!(f, "固件路径不是文件: {}", path.display()),
            Self::FlashFailed(msg) => write!(f, "烧录失败: {msg}"),
        }
    }
}

impl std::error::Error for FlashError {}

pub fn build_flash_command(port: &str, image: &Path) -> Result<FlashCommand, FlashError> {
    validate_port(port)?;
    validate_image(image)?;
    let port = port.trim();

    Ok(FlashCommand {
        program: "espflash".to_string(),
        args: vec![
            "flash".to_string(),
            "--chip".to_string(),
            "esp32s3".to_string(),
            "--port".to_string(),
            port.to_string(),
            image.to_string_lossy().to_string(),
        ],
    })
}

pub fn validate_port(port: &str) -> Result<(), FlashError> {
    let port = port.trim();
    if port.is_empty() || port == "未发现串口" || port == "扫描失败" || port == "正在扫描..."
    {
        Err(FlashError::MissingPort)
    } else {
        Ok(())
    }
}

pub fn validate_image(image: &Path) -> Result<(), FlashError> {
    if image.as_os_str().is_empty() {
        return Err(FlashError::MissingImage);
    }
    if !image.exists() {
        return Err(FlashError::ImageNotFound(image.to_path_buf()));
    }
    if !image.is_file() {
        return Err(FlashError::ImageNotFile(image.to_path_buf()));
    }
    let file_name = image
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| FlashError::InvalidImagePath(image.display().to_string()))?;
    if !file_name.ends_with("-esp32s3-merged.bin") {
        return Err(FlashError::InvalidImageExtension);
    }
    Ok(())
}

pub fn discover_ports() -> Result<Vec<PortEntry>, String> {
    let mut ports = serialport::available_ports()
        .map_err(|err| format!("串口扫描失败: {err}"))?
        .into_iter()
        .map(|port| {
            let display_name = format_port_display(&port.port_name, &port.port_type);
            PortEntry {
                name: port.port_name,
                display_name,
            }
        })
        .collect::<Vec<_>>();
    ports.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(ports)
}

pub fn is_espflash_available() -> bool {
    Command::new("espflash").arg("--version").output().is_ok()
}

pub fn prepare_flash(port: &str, image: &Path) -> Result<FlashCommand, FlashError> {
    let command = build_flash_command(port, image)?;
    if !is_espflash_available() {
        return Err(FlashError::MissingEspflash);
    }
    Ok(command)
}

pub fn flash_firmware(port: &str, image: &Path) -> Result<String, FlashError> {
    let command = prepare_flash(port, image)?;
    let output = Command::new(&command.program)
        .args(&command.args)
        .output()
        .map_err(|err| FlashError::FlashFailed(err.to_string()))?;

    flash_output_to_log(output)
}

pub fn flash_output_to_log(output: Output) -> Result<String, FlashError> {
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if output.status.success() {
        let mut lines = Vec::new();
        if !stdout.is_empty() {
            lines.push(stdout);
        }
        if !stderr.is_empty() {
            lines.push(stderr);
        }
        lines.push("烧录完成".to_string());
        Ok(lines.join("\n"))
    } else {
        let detail = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("espflash 退出码 {}", output.status)
        };
        Err(FlashError::FlashFailed(detail))
    }
}

fn format_port_display(name: &str, port_type: &serialport::SerialPortType) -> String {
    match port_type {
        serialport::SerialPortType::UsbPort(info) => {
            let mut parts = vec![
                name.to_string(),
                format!("USB {:04x}:{:04x}", info.vid, info.pid),
            ];
            if let Some(product) = &info.product {
                parts.push(product.clone());
            }
            if let Some(manufacturer) = &info.manufacturer {
                parts.push(manufacturer.clone());
            }
            parts.join(" - ")
        }
        _ => name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn build_flash_command_uses_expected_arguments() {
        let image = temp_file("inkframe-device-v0.1.0-esp32s3-merged.bin");
        let command = build_flash_command("COM3", &image).unwrap();

        assert_eq!(command.program, "espflash");
        assert_eq!(
            command.args,
            vec![
                "flash",
                "--chip",
                "esp32s3",
                "--port",
                "COM3",
                image.to_string_lossy().as_ref(),
            ]
        );
    }

    #[test]
    fn build_flash_command_trims_port() {
        let image = temp_file("inkframe-device-v0.1.0-esp32s3-merged.bin");
        let command = build_flash_command(" COM3 ", &image).unwrap();

        assert_eq!(command.args[4], "COM3");
    }

    #[test]
    fn validate_image_rejects_wrong_suffix() {
        let image = temp_file("bad.bin");
        let err = validate_image(&image).unwrap_err();
        assert_eq!(err, FlashError::InvalidImageExtension);
    }

    #[test]
    fn validate_image_accepts_expected_suffix() {
        let image = temp_file("inkframe-device-v0.1.0-esp32s3-merged.bin");
        validate_image(&image).unwrap();
    }

    #[test]
    fn validate_port_rejects_blank() {
        assert_eq!(validate_port("   ").unwrap_err(), FlashError::MissingPort);
    }

    #[test]
    fn validate_port_rejects_placeholder() {
        assert_eq!(
            validate_port("未发现串口").unwrap_err(),
            FlashError::MissingPort
        );
    }

    fn temp_file(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("flasher-{unique}-{name}"));
        File::create(&path).unwrap();
        path
    }

    #[allow(dead_code)]
    fn _cleanup(path: &Path) {
        let _ = fs::remove_file(path);
    }
}
