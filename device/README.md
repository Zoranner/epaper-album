# Inkframe Device

设备端是 ESP32-S3-PhotoPainter 固件工程，独立位于 `device/`。服务端位于 `../server/`，共享协议 crate 位于 `../crates/protocol/`。

## 构建环境

本机已验证的 ESP-IDF 版本为 `v5.5.4`。构建前先设置用户环境变量 `IDF_TOOLS_PATH`，指向 ESP-IDF 安装根目录，例如 `C:\Espressif`。本目录提供 PowerShell 激活脚本，负责在当前终端补齐 ESP-IDF、Python venv、CMake、Ninja、Xtensa 工具链、Clang 和 ROM ELF 路径。

```powershell
. .\scripts\activate-esp-idf.ps1
```

执行后，当前终端可以找到 `idf.py`、`cmake`、`ninja`、`xtensa-esp32s3-elf-gcc` 和 `libclang.dll`。

## 构建与烧录

调试构建：

```powershell
cargo +esp build --target xtensa-esp32s3-espidf
```

发布构建：

```powershell
cargo +esp build --release --target xtensa-esp32s3-espidf
```

开发阶段可以直接使用 `cargo run`，本工程已配置 runner 为 `espflash flash --monitor`。

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

## 运行数据

TF 卡根目录放置 `/sdcard/config.toml`，设备即可读取 Wi-Fi、云端地址和 `secret-key`。设备运行数据写入 `/sdcard/data/`：当前计划保存为 `plan.json`，运行状态保存为 `state.json`，图片缓存保存到 `images/{sha256}.bmp`，标题和日期 sprite 缓存保存到 `sprites/{sha256}.bmp`。

配置示例位于：

```text
examples/sdcard/config.toml
```

照片处理测试脚本可以把桌面 `sample.jpg` 转成 800x480、24-bit、未压缩 BMP：

```powershell
.\scripts\prepare-test-bmp.ps1
```

## 硬件自检

设备启动时长按 KEY 按键约 2 秒，会进入硬件自检流程。KEY 使用 GPIO4，内部上拉，低电平按下。自检会读取 TF 卡、解析 `/sdcard/config.toml`、按配置测试 Wi-Fi 和 HTTP，并刷新墨水屏。

自检屏幕保留六色色条作为底图，中间区域覆盖白底黑字的点阵报告面板，显示 `WAKE`、`STORAGE`、`CONFIG`、`WIFI`、`HTTP`、`WAKE MARKER` 和 `EPD` 状态。串口监视器同步输出同一组状态日志。

## 验证

```powershell
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

需要验证 ESP32-S3 目标构建时，先激活 ESP-IDF 环境，再执行目标构建。
