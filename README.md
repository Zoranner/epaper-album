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

TF 卡根目录放置 `/sdcard/config.toml`，设备即可读取 Wi-Fi、云端地址和 `secret-key`。设备运行数据写入 `/sdcard/data/`：当前计划保存为 `plan.json`，运行状态保存为 `state.json`，图片缓存保存到 `images/{sha256}.bmp`，标题和日期 sprite 缓存保存到 `sprites/{sha256}.bmp`。

设备遇到影响运行流程的硬错误时，会刷新内置英文错误页，覆盖 TF 卡不可用、配置缺失、低电量、同步失败和无可用图片等状态。

仓库提供 TF 卡配置示例：

```text
examples/sdcard/config.toml
```

将该文件复制到 TF 卡根目录并填写实际配置即可启动设备运行流程。

## 硬件自检

设备启动时长按 KEY 按键约 2 秒，会进入硬件自检流程。KEY 使用 GPIO4，内部上拉，低电平按下。自检会读取 TF 卡、解析 `/sdcard/config.toml`、按配置测试 Wi-Fi 和 HTTP，并刷新墨水屏。

自检屏幕保留六色色条作为底图，中间区域覆盖白底黑字的点阵报告面板，显示 `WAKE`、`STORAGE`、`CONFIG`、`WIFI`、`HTTP`、`WAKE MARKER` 和 `EPD` 状态。串口监视器同步输出同一组状态日志。

## 照片处理测试

仓库提供桌面照片处理脚本，可以把桌面 `sample.jpg` 转成 800x480、24-bit、未压缩 BMP，用于照片显示链路测试：

```powershell
.\scripts\prepare-test-bmp.ps1
```

脚本默认输出到桌面 `test.bmp`，可以作为服务端上传、TF 卡缓存和显示刷新流程的测试图片来源。

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

## 设备端验证

设备端位于仓库根目录，依赖共享协议 crate `crates/protocol`。常用验证命令在仓库根目录执行，不覆盖 `server/` 服务端工程：

```powershell
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

需要验证 ESP32-S3 目标构建时，先激活 ESP-IDF 环境，再执行目标构建：

```powershell
$env:IDF_TOOLS_PATH='C:\Espressif'
. .\scripts\activate-esp-idf.ps1
cargo +esp build --target xtensa-esp32s3-espidf
```

## 服务端构建

服务端位于 `server/`，是独立的 Rust 后端和 Vue 管理台工程。服务端通过 path dependency 引用 `../crates/protocol` 共享协议 crate，不纳入设备端 Cargo workspace。服务端 Cargo 构建会自动检查并编译 `server/web` 前端，前端依赖和构建统一使用 `bun`。

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
$env:SKIP_FRONTEND_BUILD = "1"
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features

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

然后编辑 `server/.env` 中的 `SECRET_KEY`、`ADMIN_USERNAME` 和 `ADMIN_PASSWORD`。服务端生产模式会拒绝缺失值、开发默认值和 `change-me` 占位值。`server/.env` 只用于本地或部署环境，不纳入版本管理。

sprite 生成接口需要配置 `TEXT_FONT_PATH`，指向服务端可读取的 TTF、OTF 或 TTC 字体文件。容器部署时先把字体文件挂载到容器内，再在 `server/.env` 中写入对应路径：

```env
TEXT_FONT_PATH=/app/fonts/NotoSansCJK-Regular.ttc
```

该配置只影响 sprite 生成接口；未配置字体文件时，照片计划、图片上传、图片处理和设备同步等服务端功能仍可运行。

## 标签发布

推送 `v*` 标签会触发 GitHub Actions 发布流程：

```powershell
git tag v0.1.0
git push origin v0.1.0
```

发布流程按两个工程分别处理：

- 设备端根工程使用 `cargo +esp build --release --target xtensa-esp32s3-espidf` 构建 ESP32-S3 固件，并在 GitHub Release 中上传 ELF、合并烧录镜像、bootloader、分区表和 sha256 校验文件。
- 服务端工程使用 `server/Dockerfile` 构建容器镜像，并推送到 GitHub Container Registry，镜像名为 `ghcr.io/<owner>/epaper-album-server:<tag>`，同时更新 `latest` 标签。

Release 页面中的合并烧录镜像文件名形如：

```text
epaper-album-v0.1.0-esp32s3-merged.bin
```

服务端镜像运行时仍需按部署环境提供 `SECRET_KEY`、`ADMIN_USERNAME`、`ADMIN_PASSWORD`、`DATABASE_URL` 和字体路径等配置。
