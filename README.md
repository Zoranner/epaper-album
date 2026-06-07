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

当前 ESP32-S3 固件入口会执行一次设备运行周期：读取 TF 卡配置和本地状态，按同步计划连接云端，下载计划和缺失图片，基于缓存生成显示决策，刷新屏幕后写入运行状态。烧录后串口监视器会显示带 ESP-IDF 日志前缀的内容：

```text
I (...) epaper_album: wake: unknown
I (...) epaper_album: device outcome: completed
I (...) epaper_album: cycle outcome: RefreshOnly
I (...) epaper_album: sync attempted: true
I (...) epaper_album: sync succeeded: true
I (...) epaper_album: refresh attempted: true
I (...) epaper_album: refresh succeeded: true
I (...) epaper_album: next wake: Some(...), sleep seconds: Some(...)
```

`device outcome` 表示 ESP-IDF 适配层结果，常见值包括 `completed`、`storage-mount-error`、`epd-init-error` 和 `state-write-error`。`cycle outcome` 表示业务周期结果，常见值包括 `SyncRequested`、`RefreshOnly`、`SleepOnly`、`LowBatterySkipSync`、`SyncFailed`、`RefreshFailed` 和 `NoUsablePhoto`。`next wake` 和 `sleep seconds` 来自调度计算，当前开发入口输出计划值，深度睡眠执行保持手动接入。

TF 卡根目录放置 `/sdcard/config.toml`，设备即可读取 Wi-Fi、云端地址和 `secret-key`。云端计划引用的图片缓存到 `/sdcard/epaper-album/images/`，标题、日期和通知 sprite 缓存到 `/sdcard/epaper-album/sprites/`，运行状态写入 `/sdcard/epaper-album/device-state.json`。

硬件照片自检阶段使用的 `test.bmp` 仍可作为屏幕链路排查资源。`test.bmp` 使用 `800x480`、24-bit、未压缩 BMP。仓库提供桌面照片处理脚本，可以把桌面 `sample.jpg` 转成六色屏测试图：

```powershell
.\scripts\prepare-test-bmp.ps1
```

脚本默认输出到桌面 `test.bmp`，复制到 TF 卡根目录后重新烧录或重启设备即可执行照片刷屏自检。

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

## 服务端构建

服务端位于 `server/`，是独立的 Rust 后端和 Vue 管理台工程。服务端 Cargo 构建会自动检查并编译 `server/web` 前端，前端依赖和构建统一使用 `bun`。

```powershell
cd server
cargo build --release
```

需要只编译后端时，可以设置 `SKIP_FRONTEND_BUILD=1`：

```powershell
cd server
$env:SKIP_FRONTEND_BUILD = "1"
cargo build --release
```

常用验证命令：

```powershell
cd server
cargo fmt --all
cargo test --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings

cd web
bun run build
```

## 服务端容器部署

服务端提供 Docker 多阶段构建。镜像构建会先用 `bun` 编译管理台，再编译 Rust 后端，运行容器中挂载 `/app/data` 保存 SQLite 数据库、原始图片和显示 BMP。

```bash
cd server
./docker-build.sh
```

默认镜像名为 `epaper-album-server:latest`，也可以传入 tag：

```bash
./docker-build.sh 0.1.0
```

`server/docker/docker-compose.yml` 提供基础部署配置，默认暴露 `3000` 端口，并通过环境变量配置设备密钥和管理员账号密码。正式部署时应调整：

```bash
cd server
cp .env.example .env
```

然后编辑 `server/.env` 中的 `SECRET_KEY`、`ADMIN_USERNAME` 和 `ADMIN_PASSWORD`。`server/.env` 只用于本地或部署环境，不纳入版本管理。

sprite 生成接口需要配置 `TEXT_FONT_PATH`，指向服务端可读取的 TTF、OTF 或 TTC 字体文件。容器部署时先把字体文件挂载到容器内，再在 `server/.env` 中写入对应路径：

```env
TEXT_FONT_PATH=/app/fonts/NotoSansCJK-Regular.ttc
```

该配置只影响 sprite 生成接口；未配置字体文件时，照片计划、图片上传、图片处理和设备同步等服务端功能仍可运行。
