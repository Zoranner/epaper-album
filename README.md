# ESP32-S3 电子墨水相册

本仓库采用单仓库双工程结构：设备端固件位于 `device/`，服务端位于 `server/`，共享协议 crate 位于 `crates/protocol/`。

```text
epaper-album/
  device/            # ESP32-S3 固件工程
  server/            # Rust 服务端和 Vue 管理台工程
  crates/protocol/   # 设备端和服务端共享协议契约
  docs/              # 产品级和跨工程文档
```

`device/` 和 `server/` 是两个独立 Rust 工程。日常构建、验证和发布排查应分别进入对应目录执行，避免把一个工程的通过误判成全仓通过。

## 设备端

设备端使用 Rust 开发，目标硬件为 ESP32-S3-PhotoPainter。本机已验证的 ESP-IDF 版本为 `v5.5.4`。构建前先设置用户环境变量 `IDF_TOOLS_PATH`，指向 ESP-IDF 安装根目录，例如 `C:\Espressif`。

```powershell
cd device
. .\scripts\activate-esp-idf.ps1
```

调试构建：

```powershell
cd device
cargo +esp build --target xtensa-esp32s3-espidf
```

发布构建：

```powershell
cd device
cargo +esp build --release --target xtensa-esp32s3-espidf
```

烧录并串口监视：

```powershell
cd device
cargo +esp run --release --target xtensa-esp32s3-espidf
```

指定串口烧录：

```powershell
cd device
cargo +esp espflash flash --release --target xtensa-esp32s3-espidf --monitor --port COM3
```

设备端产物位于 `device/target/xtensa-esp32s3-espidf/`。常见 release 产物包括：

```text
device/target/xtensa-esp32s3-espidf/release/epaper-album
device/target/xtensa-esp32s3-espidf/release/bootloader.bin
device/target/xtensa-esp32s3-espidf/release/partition-table.bin
```

设备端验证：

```powershell
cd device
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

设备 TF 卡配置示例位于 `device/examples/sdcard/config.toml`。照片处理测试脚本位于 `device/scripts/prepare-test-bmp.ps1`。

## 服务端

服务端位于 `server/`，是独立 Rust 后端和 Vue 管理台工程。服务端通过 `../crates/protocol` 引用共享协议 crate。服务端 Cargo 构建会自动检查并编译 `server/web` 前端，前端依赖和构建统一使用 `bun`。

```powershell
cd server
cargo build --release
```

只验证后端时，可以跳过前端构建：

```powershell
cd server
$env:SKIP_FRONTEND_BUILD = "1"
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

前端构建：

```powershell
cd server\web
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

- 设备端工程进入 `device/`，使用 `cargo +esp build --release --target xtensa-esp32s3-espidf` 构建 ESP32-S3 固件，并在 GitHub Release 中上传 ELF、合并烧录镜像、bootloader、分区表和 sha256 校验文件。
- 服务端工程使用 `server/Dockerfile` 构建容器镜像，并推送到 GitHub Container Registry，镜像名为 `ghcr.io/<owner>/epaper-album-server:<tag>`，同时更新 `latest` 标签。

Release 页面中的合并烧录镜像文件名形如：

```text
epaper-album-v0.1.0-esp32s3-merged.bin
```

服务端镜像运行时仍需按部署环境提供 `SECRET_KEY`、`ADMIN_USERNAME`、`ADMIN_PASSWORD`、`DATABASE_URL` 和字体路径等配置。
