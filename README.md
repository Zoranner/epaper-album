# ESP32-S3 电子墨水相册固件

## 构建环境

本项目使用 Rust 开发，目标硬件为 ESP32-S3-PhotoPainter。

本机已验证的 ESP-IDF 版本为 `v5.5.4`。构建前先设置用户环境变量 `IDF_TOOLS_PATH`，指向 ESP-IDF 安装根目录，例如 `C:\Espressif`。仓库提供 PowerShell 激活脚本，负责在当前终端补齐 ESP-IDF、Python venv、CMake、Ninja、Xtensa 工具链、Clang 和 ROM ELF 路径。

```powershell
. .\scripts\activate-esp-idf.ps1
```

执行后，当前终端可以找到 `idf.py`、`cmake`、`ninja`、`xtensa-esp32s3-elf-gcc` 和 `libclang.dll`。

仓库配置固定使用 ESP-IDF 5.5.4，并要求使用当前终端激活的 ESP-IDF 环境：

```toml
[env]
MCU = "esp32s3"
ESP_IDF_VERSION = "tag:v5.5.4"
ESP_IDF_TOOLS_INSTALL_DIR = "fromenv"
ESP_IDF_PATH_ISSUES = "warn"
```

`ESP_IDF_PATH_ISSUES = "warn"` 用于处理 `esp-idf-sys` 在 Windows 下的路径长度预检。`IDF_PATH` 由激活脚本写入当前终端环境。

## 调试构建

Cargo 默认使用 dev profile，产物位于 `target/xtensa-esp32s3-espidf/debug/`。

```powershell
cargo +esp build --target xtensa-esp32s3-espidf
```

调试构建用于工具链验证和开发调试。长期运行固件使用 release profile。

## 发布构建

长期运行的固件使用 release profile：

```powershell
cargo +esp build --release --target xtensa-esp32s3-espidf
```

发布构建产物位于 `target/xtensa-esp32s3-espidf/release/`。

## 烧录与串口监视

开发阶段可以直接使用 `cargo run`，仓库已配置 runner 为 `espflash flash --monitor`。

```powershell
cargo +esp run --target xtensa-esp32s3-espidf
```

烧录发布版：

```powershell
cargo +esp run --release --target xtensa-esp32s3-espidf
```

如果电脑连接了多个串口，使用 `cargo-espflash` 指定端口：

```powershell
cargo +esp espflash flash --release --target xtensa-esp32s3-espidf --monitor --port COM3
```

当前固件入口会执行串口自检，输出配置文件探测结果和渲染探测结果。烧录后串口监视器会显示带 ESP-IDF 日志前缀的内容：

```text
I (...) epaper_album: epaper-album self-test
I (...) epaper_album: storage: available
I (...) epaper_album: config: missing
I (...) epaper_album: render refresh count: 0
I (...) epaper_album: render sleep: false
```

`storage` 的取值包括 `available` 和 `mount-error`。`config` 的取值包括 `valid`、`incomplete`、`missing`、`parse-error` 和 `read-error`。TF 卡根目录提供 `config.toml` 后，可以通过这两项确认设备端 TF 卡挂载和配置文件读取状态。

## 产物说明

构建后会生成应用 ELF、bootloader 和分区表。烧录流程由 `espflash` 根据 ELF 和 ESP-IDF 构建产物统一处理烧录参数。

常见产物路径：

```text
target/xtensa-esp32s3-espidf/debug/epaper-album
target/xtensa-esp32s3-espidf/debug/bootloader.bin
target/xtensa-esp32s3-espidf/debug/partition-table.bin
```

release 构建时对应目录为：

```text
target/xtensa-esp32s3-espidf/release/
```

`libespidf.bin` 属于 ESP-IDF 侧支撑产物，主固件烧录入口使用应用 ELF。
