# Inkframe Flasher

Inkframe Flasher 是独立的 Windows 桌面烧录工具，用于把发布产物中的 ESP32-S3 合并镜像写入设备。

第一版边界：

- 只选择本地 `*-esp32s3-merged.bin` 文件。
- 只调用 PATH 中的 `espflash` 执行烧录。
- 不编译设备端固件。
- 不管理服务端。
- 不写入 TF 卡配置。

烧录命令等价于：

```powershell
espflash flash --chip esp32s3 --port COM3 path\to\inkframe-device-v0.1.0-esp32s3-merged.bin
```

开发验证：

```powershell
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
```
