# Epaper Album 服务端设计文档

## 服务端职责

服务端放在仓库 `server/` 子目录中，作为独立 Rust 工程管理。当前实现提供电子相册设备需要的计划接口和图片资源接口，同时提供一个 Vue 管理台用于维护计划、上传图片和查看资源列表。

服务端按设备端现有设计保持最小数据面。设备只关心 `version + plans`，计划项只包含 `start`、`end`、`caption` 和图片 `sha256` 列表。图片下载地址由设备使用本地 `base_url` 和 `sha256` 组合得到，计划响应中不写图片 URL。

## 运行配置

服务端启动入口为 `server/src/main.rs`，运行时配置来自环境变量。

| 变量 | 默认值 | 用途 |
| --- | --- | --- |
| `LISTEN_PORT` | `3000` | HTTP 服务监听端口 |
| `DATABASE_URL` | `sqlite:data/epaper-album.db?mode=rwc` | SQLite 数据库连接地址 |
| `EPAPER_ALBUM_SECRET_KEY` | `local-secret-key` | 设备和管理端请求接口时使用的密钥 |

启动时服务端会创建 `data/` 目录，初始化 SQLite 表结构，然后挂载 API 路由和 `web/dist` 静态前端目录。`server/data/`、`server/target/`、`server/web/dist/` 和 `server/web/node_modules/` 已作为本地运行产物忽略。

## 鉴权规则

除健康检查和静态前端页面外，接口统一使用请求头 `secret-key` 鉴权。请求头值需要与服务端 `EPAPER_ALBUM_SECRET_KEY` 一致。

```http
secret-key: local-secret-key
```

鉴权失败返回：

```json
{
  "error": "Invalid secret-key"
}
```

对应 HTTP 状态码为 `401 Unauthorized`。当前实现没有多用户、设备绑定和密钥轮换机制，密钥由服务端部署配置统一管理。

`PUT /api/manifest` 和 `POST /api/images` 分别使用 Axum 的 JSON、multipart extractor 解析请求体。请求体格式或 `Content-Type` 明显不符合 extractor 要求时，框架可能在进入业务 handler 前返回解析错误；鉴权规则适用于成功进入业务 handler 的接口处理流程。

## 接口设计

### 健康检查

```http
GET /api/healthz
```

用途：部署探活和本地运行检查。

响应：

```text
ok
```

该接口不需要 `secret-key`。

### 获取计划

```http
GET /api/manifest
```

用途：设备端同步计划，管理台加载当前计划。

请求头：

```http
secret-key: local-secret-key
```

响应：

```json
{
  "version": "2026-06-06-001",
  "plans": [
    {
      "start": "2026-06-06",
      "end": "2026-06-06",
      "caption": "晚风和海",
      "images": [
        "7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069"
      ]
    }
  ]
}
```

字段说明：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `version` | string | 当前计划版本，设备用于判断计划是否变化 |
| `plans` | array | 计划列表 |
| `plans[].start` | string | 计划开始日期，当前按 `YYYY-MM-DD` 字符串处理 |
| `plans[].end` | string | 计划结束日期，包含当天 |
| `plans[].caption` | string | 设备左下角标题 |
| `plans[].images` | string[] | 图片资源 `sha256` 列表 |

数据库没有计划数据时，接口返回：

```json
{
  "version": "0",
  "plans": []
}
```

### 更新计划

```http
PUT /api/manifest
```

用途：管理台保存完整计划。当前实现采用整体替换模式，提交的新 manifest 会替换全部计划行和版本号。

请求头：

```http
secret-key: local-secret-key
Content-Type: application/json
```

请求体与 `GET /api/manifest` 响应结构一致。

校验规则：

- `version` 去除空白后需要非空。
- 每个计划项的 `start` 和 `end` 需要非空。
- 每个计划项至少包含一个图片 `sha256`。
- 每个 `sha256` 必须是 64 位十六进制字符串。

成功响应返回保存后的 manifest。校验失败返回 `400 Bad Request`，响应体格式为：

```json
{
  "error": "Invalid sha256: xxx"
}
```

### 上传图片

```http
POST /api/images
```

用途：管理台上传图片资源。服务端读取 multipart 字段 `image`，按文件内容计算 SHA-256，并把图片内容写入 SQLite。

请求头：

```http
secret-key: local-secret-key
Content-Type: multipart/form-data
```

表单字段：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `image` | file | 图片文件内容 |

成功响应状态码为 `201 Created`：

```json
{
  "sha256": "7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069",
  "url": "/images/7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069"
}
```

写入规则：

- 服务端以文件内容计算 `sha256`，不信任客户端传入的文件名或摘要。
- 同一 `sha256` 重复上传时执行 upsert，更新 `content_type` 和二进制内容。
- 空文件返回 `400 Bad Request`。
- 没有 `image` 字段返回 `400 Bad Request`。

### 图片列表

```http
GET /api/images
```

用途：管理台查看当前已保存资源。

请求头：

```http
secret-key: local-secret-key
```

响应：

```json
[
  {
    "sha256": "7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069",
    "content_type": "image/bmp",
    "size": 123456,
    "url": "/images/7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069"
  }
]
```

列表按 `sha256` 升序返回。`size` 来自 SQLite 中二进制字段的 `length(bytes)`。

### 删除图片

```http
DELETE /api/images/:sha256
```

用途：管理台删除未使用或错误上传的图片资源。

请求头：

```http
secret-key: local-secret-key
```

路径参数：

| 参数 | 说明 |
| --- | --- |
| `sha256` | 64 位十六进制图片摘要 |

成功删除返回 `204 No Content`。资源不存在返回 `404 Not Found`。`sha256` 格式不正确返回 `400 Bad Request`。

### 下载图片

```http
GET /images/:sha256
```

用途：设备端按 `sha256` 下载图片资源；管理台预览图片时也使用该接口。

请求头：

```http
secret-key: local-secret-key
```

路径参数：

| 参数 | 说明 |
| --- | --- |
| `sha256` | 64 位十六进制图片摘要 |

成功响应直接返回图片二进制内容，并设置 `Content-Type` 为上传时记录的 `content_type`。资源不存在返回 `404 Not Found`。`sha256` 格式不正确返回 `400 Bad Request`。

### 前端静态页面

```http
GET /
GET /assets/*
```

用途：访问管理台及其构建产物。服务端把未命中的请求交给 `web/dist` 静态文件服务，并在目录访问时追加 `index.html`。当前实现不是专门的 SPA history fallback，未实际存在的深层路径不保证返回 `index.html`。

管理台本身不绕过接口鉴权。用户在页面中输入管理密钥后，浏览器把密钥写入本地存储，并在调用 API 时通过 `secret-key` 请求头发送。图片预览不能通过普通 `<img>` 标签携带请求头，因此管理台使用 `fetch` 带密钥读取图片，再生成临时 `blob:` URL 进行预览。

## 数据库设计

当前数据库使用 SQLite，由 `server/src/db.rs` 在启动时自动创建表结构。数据库默认路径为 `server/data/epaper-album.db`。

### `plan_versions`

保存当前 manifest 的版本号。当前系统只有一个活动 manifest，因此表中固定使用 `id = 1`。

```sql
CREATE TABLE IF NOT EXISTS plan_versions (
    id      INTEGER PRIMARY KEY CHECK (id = 1),
    version TEXT NOT NULL
);
```

字段说明：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `id` | INTEGER | 固定为 `1`，用于约束单一活动版本 |
| `version` | TEXT | 当前计划版本 |

### `plan_entries`

保存 manifest 中的计划项。每次更新 manifest 时，服务端在事务中清空旧计划项并重新插入新计划项。

```sql
CREATE TABLE IF NOT EXISTS plan_entries (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    start_date  TEXT NOT NULL,
    end_date    TEXT NOT NULL,
    caption     TEXT NOT NULL,
    images_json TEXT NOT NULL,
    position    INTEGER NOT NULL
);
```

字段说明：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `id` | INTEGER | 自增主键 |
| `start_date` | TEXT | 计划开始日期，对应接口字段 `start` |
| `end_date` | TEXT | 计划结束日期，对应接口字段 `end` |
| `caption` | TEXT | 标题，对应接口字段 `caption` |
| `images_json` | TEXT | 图片 `sha256` 字符串数组的 JSON 序列化结果 |
| `position` | INTEGER | manifest 内原始顺序，用于同一天多条计划的稳定排序 |

读取 manifest 时，服务端按以下规则排序：

```sql
ORDER BY start_date ASC, position ASC, id ASC
```

这个排序让设备优先按日期读取计划，同时保留同一开始日期下管理台提交时的顺序。

### `images`

保存图片资源的摘要、内容类型和二进制内容。

```sql
CREATE TABLE IF NOT EXISTS images (
    sha256       TEXT PRIMARY KEY,
    content_type TEXT NOT NULL,
    bytes        BLOB NOT NULL
);
```

字段说明：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `sha256` | TEXT | 图片内容 SHA-256 摘要，作为资源主键 |
| `content_type` | TEXT | 上传时记录的 MIME 类型 |
| `bytes` | BLOB | 图片二进制内容 |

图片列表接口不直接返回 `bytes`，只通过 `length(bytes)` 计算资源大小。图片下载接口按 `sha256` 查询并返回 `bytes`。

## 写入流程

### 保存 manifest

管理台提交完整 manifest 后，后端执行一次事务：

- 删除 `plan_entries` 中旧计划。
- 删除 `plan_versions` 中旧版本。
- 插入 `id = 1` 的新版本。
- 按请求中的计划顺序写入 `plan_entries`，并保存 `position`。
- 提交事务。

该流程保证 manifest 读取时只看到一组完整计划。

### 上传图片

管理台上传图片后，后端执行以下处理：

- 读取 multipart 字段 `image`。
- 计算图片内容的 SHA-256 十六进制字符串。
- 保存 `sha256`、`content_type` 和 `bytes`。
- 如果 `sha256` 已存在，更新已有记录。
- 返回 `sha256` 和下载路径。

设备下载后仍需要用本地计算出的 SHA-256 校验图片内容，校验通过后以 `sha256` 作为缓存文件名。

## 前端管理台

管理台位于 `server/web`，使用 Vue 3、Vite、TypeScript 和 bun。页面是工作台式布局，主要功能包括：

- 配置并保存管理密钥。
- 查看当前 manifest 的版本号、计划数量和 JSON 预览。
- 新增、编辑、删除计划项。
- 上传图片并显示返回的 `sha256`。
- 查看图片资源列表、大小和预览。
- 删除图片资源。

前端开发代理在 `server/web/vite.config.ts` 中配置，开发模式下 `/api` 和 `/images` 转发到 `http://localhost:3000`。

## 验证覆盖

当前服务端测试位于 `server/tests/core.rs`，覆盖以下内容：

- manifest JSON 结构与设备契约一致，不包含 `url` 和 `base_url`。
- 计划持久化、读取排序和整体替换行为。
- 图片元数据和二进制内容持久化。
- 错误响应 JSON 结构。
- `/api/manifest` 的 `secret-key` 鉴权和响应结构。
- `/images/:sha256` 的 `secret-key` 鉴权、`Content-Type` 和二进制内容返回。

建议使用以下命令验证：

```powershell
cd server
cargo fmt --all
cargo test --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings

cd web
bun run build
```
