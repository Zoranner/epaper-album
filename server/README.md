# Inkframe Server

服务端是独立 Rust 后端和 Vue 管理台工程，目录边界在 `server/`。服务端通过 `../crates/protocol` 引用共享协议 crate，不纳入设备端 Cargo workspace。

## 本地验证

```powershell
cd server
$env:SKIP_FRONTEND_BUILD = "1"
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features

cd web
bun run build
```

## 发布构建

完整服务端 Cargo 构建会通过 `build.rs` 自动执行 `bun install --frozen-lockfile` 和 `bun run build`。

```powershell
cd server
cargo build --release
```

容器镜像通过 `server/docker-build.sh` 发起构建，构建上下文为仓库根目录，用于同时复制 `server/` 和 `crates/protocol/`。

```bash
cd server
./docker-build.sh
```

## 部署配置

复制 `.env.example` 为 `.env` 后设置真实密钥和管理员账号密码。生产模式会拒绝缺失值、开发默认值和 `change-me` 占位值。
