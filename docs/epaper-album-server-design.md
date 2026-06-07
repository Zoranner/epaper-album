# Epaper Album 服务端设计文档

## 服务端职责

服务端放在仓库 `server/` 子目录中，作为独立工程管理。服务端负责维护照片计划、接收原始图片、排队生成电子墨水屏可直接使用的 BMP 图片，并向设备提供最近几天的计划和显示图片资源。

设备端只读取计划和显示图片。计划接口围绕单条计划管理。设备每次同步都按接口结果覆盖本地计划。

## 运行配置

服务端启动入口为 `server/src/main.rs`，运行时配置来自环境变量。

| 变量 | 默认值 | 用途 |
| --- | --- | --- |
| `LISTEN_PORT` | `3000` | HTTP 服务监听端口 |
| `DATABASE_URL` | `sqlite:data/epaper-album.db?mode=rwc` | SQLite 数据库连接地址 |
| `SECRET_KEY` | `local-secret-key` | 设备和用户权限请求接口时使用的密钥 |
| `ADMIN_USERNAME` | `admin` | 管理员账号 |
| `ADMIN_PASSWORD` | `admin` | 管理员密码 |

`SECRET_KEY` 用于设备同步计划和下载显示图片。管理员账号密码用于管理台登录和管理接口权限。服务端启动时创建 `data/`、`data/images/original/`、`data/images/display/` 和 `data/sprites/` 目录，初始化 SQLite 表结构，并挂载 API 路由和管理台静态文件。

`server/.env.example` 提供本地和容器部署的环境变量示例。实际部署时复制为 `server/.env` 并调整密钥和管理员密码；`server/.env` 不纳入版本管理。

sprite 生成接口读取 `server/assets/fonts.toml` 和 `server/assets/fonts/` 下的字体资源，并用字体 rasterize 方式生成小尺寸黑白 BMP。`fonts.toml` 配置字体 fallback 顺序、字号和 padding；仓库只提供 `server/assets/fonts.example.toml`，部署或本地运行前复制为 `fonts.toml` 并把字体文件放入固定目录。真实配置和字体文件不纳入版本管理。

## 鉴权规则

除健康检查和静态前端页面外，接口统一鉴权。设备和用户权限使用请求头 `secret-key`，请求头值需要与服务端 `SECRET_KEY` 一致。管理员权限使用管理员账号密码登录后获得的会话或令牌。

```http
secret-key: local-secret-key
```

接口 JSON 响应使用统一结构：

```json
{
  "code": 0,
  "message": "ok",
  "data": {}
}
```

`code = 0` 表示成功，非零表示失败。失败响应同样使用 JSON：

```json
{
  "code": 401,
  "message": "Unauthorized",
  "data": null
}
```

健康检查和显示图片下载是例外：`GET /api/healthz` 返回纯文本，`GET /images/:sha256` 成功时返回 BMP 二进制。显示图片下载失败时仍返回统一 JSON 错误。鉴权失败返回 `401 Unauthorized`。当前设计按个人相册服务处理，设备使用用户权限读取计划；管理台使用管理员权限维护图片和计划。

接口地址不区分用户端和管理端。同一个接口根据认证结果控制可执行动作和可见字段：用户权限只能读取计划和下载显示图片；管理员权限可以读取完整计划信息、维护计划、维护图片和下载显示图片。

## 数据对象

### 计划

计划描述某个日期范围内设备应显示的标题和图片。计划表直接保存图片 `sha256` 列表。原图保存为 `data/images/original/{sha256}`，显示 BMP 保存为 `data/images/display/{sha256}`。管理台按图片 `status` 显示处理状态；设备接口只返回 `status = 'ready'` 的图片 `sha256`。

```json
{
  "id": 1,
  "start": "2026-06-06",
  "end": "2026-06-06",
  "caption": "晚风和海",
  "images": [
    "f1d2d2f924e986ac86fdf7b36c94bcdf32beec15..."
  ]
}
```

字段说明：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `id` | integer | 计划主键 |
| `start` | string | 计划开始日期，格式为 `YYYY-MM-DD` |
| `end` | string | 计划结束日期，包含当天 |
| `caption` | string | 设备左下角标题 |
| `images` | string[] | 图片 `sha256` 列表 |

### 图片

图片上传后先按原始文件内容计算 `sha256`，再检查数据库记录和 `data/images/original/{sha256}` 文件。已有图片复用现有记录和文件；新图片保存原始文件，数据库写入 `pending` 状态，并由服务端后台任务生成适配 800 x 480 六色电子墨水屏的 BMP 文件。显示 BMP 保存为 `data/images/display/{sha256}`。数据库保存图片 `sha256`、处理状态和备注。

```json
{
  "sha256": "f1d2d2f924e986ac86fdf7b36c94bcdf32beec15...",
  "status": "pending",
  "remark": "海边晚风"
}
```

`status` 是图片处理状态的唯一依据。`ready` 表示图片已经处理完成，可以从 `data/images/display/{sha256}` 下载；`pending` 和 `processing` 表示仍在处理流程中；`failed` 表示处理失败，等待管理员重新上传或后续手动重试。

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

### 管理员登录

```http
POST /api/login
```

用途：管理台登录。

请求体：

```json
{
  "username": "admin",
  "password": "admin"
}
```

服务端使用 `ADMIN_USERNAME` 和 `ADMIN_PASSWORD` 校验账号密码。登录成功后返回 `jwtToken` 和过期时间，后续管理操作使用该 `jwtToken` 获得管理员权限。

成功响应：

```json
{
  "code": 0,
  "message": "ok",
  "data": {
    "jwtToken": "jwtToken",
    "expiresAt": "2026-06-08T12:00:00Z"
  }
}
```

### 获取计划

```http
GET /api/plans?days=3
```

用途：读取计划。设备和管理台使用同一个接口。

用户权限请求头：

```http
secret-key: local-secret-key
```

管理员权限请求头：

```http
Authorization: Bearer <admin-token>
```

查询参数：

| 参数 | 默认值 | 说明 |
| --- | --- | --- |
| `days` | `3` | 从当前日期开始返回的天数，最大值为 `7` |

服务端按本地日期筛选计划。默认返回今天起三天内的计划，`days=7` 返回今天起七天内的计划；时间范围为 `[today, today + days - 1]`。`days` 统一按宽容规则处理：缺省或不是整数时使用默认值 `3`，小于 `1` 按 `1`，大于 `7` 按 `7`。

用户权限响应：

```json
{
  "code": 0,
  "message": "ok",
  "data": [
    {
      "id": 1,
      "start": "2026-06-06",
      "end": "2026-06-06",
      "caption": "晚风和海",
      "images": [
        "f1d2d2f924e986ac86fdf7b36c94bcdf32beec15..."
      ]
    }
  ]
}
```

管理员权限响应：

```json
{
  "code": 0,
  "message": "ok",
  "data": [
    {
      "id": 1,
      "start": "2026-06-06",
      "end": "2026-06-06",
      "caption": "晚风和海",
      "images": [
        {
          "sha256": "f1d2d2f924e986ac86fdf7b36c94bcdf32beec15...",
          "status": "pending",
          "remark": "海边晚风"
        }
      ]
    }
  ]
}
```

同一个计划接口根据权限返回不同图片结构。用户权限下，`images` 只包含已经生成显示 BMP 的 `sha256`。管理员权限下，`images` 包含 `sha256`、`status` 和 `remark`，用于计划管理页面展示和选择图片。

计划接口用 SQLite `json_each(plans.images)` 展开图片摘要数组，并通过 `images.sha256` 主键关联图片状态和备注。管理员权限下保留全部图片；用户权限下只返回 `status = 'ready'` 的图片。

### 获取图片

```http
GET /api/images?keyword=海边
```

用途：图片管理页查看已有图片，计划管理页选择已有图片。该接口需要管理员权限。

管理员权限请求头：

```http
Authorization: Bearer <admin-token>
```

查询参数：

| 参数 | 说明 |
| --- | --- |
| `keyword` | 可选，按备注进行模糊搜索 |

响应：

```json
{
  "code": 0,
  "message": "ok",
  "data": [
    {
      "sha256": "f1d2d2f924e986ac86fdf7b36c94bcdf32beec15...",
      "status": "pending",
      "remark": "海边晚风"
    }
  ]
}
```

### 新增计划

```http
POST /api/plans
```

用途：新增一条计划。该接口需要管理员权限。

管理员权限请求头：

```http
Authorization: Bearer <admin-token>
Content-Type: application/json
```

请求体：

```json
{
  "start": "2026-06-06",
  "end": "2026-06-06",
  "caption": "晚风和海",
  "images": [
    "f1d2d2f924e986ac86fdf7b36c94bcdf32beec15..."
  ]
}
```

`images` 填写已有图片的 `sha256` 列表。计划管理不上传图片，只从图片管理页已经上传的图片中选择。服务端把这些摘要写入 `plans.images`。计划创建不依赖图片是否已经生成显示 BMP，也允许 `images` 为空数组，便于先维护日期和标题。

成功响应返回创建后的管理台计划视图：

```json
{
  "code": 0,
  "message": "ok",
  "data": {
    "id": 1,
    "start": "2026-06-06",
    "end": "2026-06-06",
    "caption": "晚风和海",
    "images": [
      {
        "sha256": "f1d2d2f924e986ac86fdf7b36c94bcdf32beec15...",
        "status": "pending",
        "remark": "海边晚风"
      }
    ]
  }
}
```

服务端校验 `images` 中的每个 `sha256` 已存在于 `images` 表。存在未知图片时返回 `400 Bad Request`。`images` 可以为空数组。服务端按提交顺序自动去重，重复的 `sha256` 只保留第一次出现的位置。

### 更新计划

```http
PUT /api/plans/:id
```

用途：修改单条计划。该接口需要管理员权限。

管理员权限请求头：

```http
Authorization: Bearer <admin-token>
Content-Type: application/json
```

请求体与新增计划一致。服务端校验 `images` 中的每个 `sha256` 已存在于 `images` 表。成功响应返回更新后的计划。计划不存在返回 `404 Not Found`，存在未知图片时返回 `400 Bad Request`。`images` 可以为空数组。服务端按提交顺序自动去重，重复的 `sha256` 只保留第一次出现的位置。

### 删除计划

```http
DELETE /api/plans/:id
```

用途：删除单条计划。该接口需要管理员权限。

管理员权限请求头：

```http
Authorization: Bearer <admin-token>
```

成功删除返回：

```json
{
  "code": 0,
  "message": "ok",
  "data": null
}
```

计划不存在返回 `404 Not Found`。

### 上传图片

```http
POST /api/images
```

用途：图片管理页上传原始图片。该接口需要管理员权限。服务端先按文件内容计算 `sha256`，已有图片直接复用；新图片保存原始文件后，把图片处理任务加入后台队列。

管理员权限请求头：

```http
Authorization: Bearer <admin-token>
Content-Type: multipart/form-data
```

表单字段：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `image` | file | 原始图片文件 |
| `remark` | string | 可选备注，用于后台搜索和人工识别 |

成功响应状态码为 `201 Created`：

```json
{
  "code": 0,
  "message": "ok",
  "data": {
    "sha256": "f1d2d2f924e986ac86fdf7b36c94bcdf32beec15...",
    "status": "pending",
    "remark": "海边晚风"
  }
}
```

写入规则：

- 服务端按原始文件内容计算 `sha256`，`sha256` 同时作为原始文件名和显示文件名。
- 先检查 `images.sha256` 记录和 `data/images/original/{sha256}` 文件。
- 已存在相同 `sha256` 时复用已有图片记录；表单提交了 `remark` 时更新备注，未提交时保留原备注；如果 `status` 是 `pending` 或 `processing`，确认图片处理任务已进入队列；如果 `status` 是 `failed`，把状态改回 `pending` 后入队重试。
- 不存在相同 `sha256` 时，原始图片保存到 `data/images/original/{sha256}`，数据库写入 `images(sha256, status, remark)`，初始状态为 `pending`。
- 后台任务从原始图片生成 800 x 480 BMP。
- 显示 BMP 保存到 `data/images/display/{sha256}`。

### 生成 Sprite

```http
GET /api/sprite?type=caption&text=晚风和海
```

用途：根据短文本和类型即时生成设备叠加文字使用的 BMP 小图块，供调用方预览、保存或合成显示资源时使用。该接口支持管理员权限和 `secret-key` 权限。

用户权限请求头：

```http
secret-key: local-secret-key
```

管理员权限请求头：

```http
Authorization: Bearer <admin-token>
```

查询参数：

字段说明：

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `type` | string | sprite 类型，取值为 `caption`、`date`、`notice` 或 `status` |
| `text` | string | 需要生成的小图块文字，URL 编码后传入 |

服务端按 GET 语义处理该接口，请求不会写入数据库、不会创建图片记录。成功响应返回 BMP 二进制内容，`Content-Type` 固定为 `image/bmp`。失败响应使用统一 JSON 结构。

参数规则：

- `type` 仅支持 `caption`、`date`、`notice` 和 `status`。
- `text` 去除首尾空白后不能为空。
- `text` 最多 64 个 Unicode 字符。
- 未知 `type`、空文本和超长文本返回 `400 Bad Request`。

输出规格：

| 配置项 | 值 |
| --- | --- |
| 字号 | 32px |
| 内边距 | x=12, y=8 |
| 尺寸 | 按文字内容自适应宽高 |
| 格式 | 24-bit BMP |

`type` 对齐设备端文字用途：`caption` 对应左下角标题，`date` 对应右下角日期，`notice` 对应左上角通知，`status` 对应后续右上角状态扩展。当前四种类型使用同一套字体、字号、内边距和颜色规则，`type` 用于调用方区分用途和后续扩展。生成结果为白底黑字。字体栅格化后按阈值压成纯黑白像素，不输出灰度抗锯齿像素。

这里的 `status` 指右上角扩展 sprite 类型。图片处理状态使用 `images.status` 字段表达，取值为 `pending`、`processing`、`ready` 和 `failed`。

缓存规则：

- 服务端不做数据库缓存。
- 生成结果不写入 `images` 表。
- sprite 缓存文件保存到 `data/sprites/{sha256}.bmp`。
- `sha256` 使用 `type`、文字内容和 `fonts.toml` 配置内容计算。
- 命中缓存文件时直接返回 BMP；未命中时生成 BMP，写入缓存文件后返回。

生成流程读取 `assets/fonts.toml` 中的字体文件顺序和文字样式配置，逐字符选择第一个包含对应字形的字体，使用 fontdue 这类轻量 Rust 字体 rasterizer 将文字栅格化，再按阈值压成黑白像素并输出 BMP。字体目录随工程保留为空目录，具体字体文件由运行环境自行提供。当前方案面向标题、日期和通知这类短文本，负责生成文字 sprite 小 BMP。Skia 适合复杂排版、矢量绘制和更完整图形管线，后续出现这类需求时再评估引入成本。

### 更新图片备注

```http
PUT /api/images/:sha256
```

用途：图片管理页修改图片备注。该接口需要管理员权限。

管理员权限请求头：

```http
Authorization: Bearer <admin-token>
Content-Type: application/json
```

请求体：

```json
{
  "remark": "海边晚风"
}
```

成功响应返回更新后的图片信息：

```json
{
  "code": 0,
  "message": "ok",
  "data": {
    "sha256": "f1d2d2f924e986ac86fdf7b36c94bcdf32beec15...",
    "status": "ready",
    "remark": "海边晚风"
  }
}
```

### 下载显示图片

```http
GET /images/:sha256
```

用途：设备端和管理台按显示图片文件键下载 BMP 文件。

用户权限请求头：

```http
secret-key: local-secret-key
```

管理员权限请求头：

```http
Authorization: Bearer <admin-token>
```

路径参数：

| 参数 | 说明 |
| --- | --- |
| `sha256` | 图片内容 SHA-256，也是显示 BMP 的文件键 |

下载前先查询 `images.status`。只有 `status = 'ready'` 时才返回 `data/images/display/{sha256}` 对应的 BMP 二进制内容，`Content-Type` 固定为 `image/bmp`。资源不存在、状态不是 `ready`，或 `ready` 但显示文件不存在时，返回 `404 Not Found`。下载接口不修改图片状态。`sha256` 格式不正确返回 `400 Bad Request`。错误响应使用统一 JSON 结构。

## 图片处理流程

图片处理任务以 `images.status` 作为持久队列状态，内存队列只负责当前进程内的调度。当前按单实例服务设计；如果后续改为多实例部署，需要把任务抢占和状态恢复改为跨实例安全的实现。处理目标是生成设备可直接缓存和显示的 BMP 文件。

队列规则：

- `pending` 表示等待处理。
- `processing` 表示当前进程正在处理。
- `ready` 表示显示 BMP 已生成，可以下载。
- `failed` 表示处理失败，不自动反复重试。
- 单实例服务启动后把上次遗留的 `processing` 改回 `pending`，再扫描 `pending` 图片加入内存队列。
- 上传图片时按同一规则判断是否需要入队。
- 内存队列维护当前进程中的待处理 `sha256` 集合，同一个 `sha256` 不重复入队。
- worker 取出任务前用条件更新抢占任务：`UPDATE images SET status = 'processing' WHERE sha256 = ? AND status = 'pending'`。更新成功才继续处理。
- 处理成功后先把 BMP 写入临时文件，再原子替换为 `data/images/display/{sha256}`，最后把状态更新为 `ready`。对外接口以 `status` 为准，不用文件是否存在判断处理状态。
- 处理失败时保留原图和 `images` 记录，把状态改为 `failed`。管理员重新上传同一图片或后续增加手动重试入口时，再把状态改回 `pending`。
- 服务重启后不依赖内存队列恢复状态，按 `images.status` 重新恢复未完成任务。
- 服务启动时执行一致性修复：`ready` 但 `data/images/display/{sha256}` 不存在时改回 `pending`；`pending`、`processing` 或 `failed` 不返回给用户计划接口。

处理步骤：

- 读取 `data/images/original/{sha256}`。
- 解码原始图片。
- 按 800 x 480 画布裁剪或居中适配。
- 转换为六色电子墨水屏可用颜色。
- 进行抖动处理。
- 输出 BMP。
- 写入临时 BMP 文件。
- 原子替换为 `data/images/display/{sha256}`。

管理台计划列表按 `status` 判断状态。`pending` 和 `processing` 显示“处理中”；`failed` 显示“处理失败”；`ready` 显示最终图片，并可把该图片纳入设备计划。

## 数据库设计

当前数据库使用 SQLite，由 `server/src/db.rs` 在启动时自动创建表结构。数据库默认路径为 `server/data/epaper-album.db`。

### `plans`

保存计划基本信息。

```sql
CREATE TABLE IF NOT EXISTS plans (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    start_date TEXT NOT NULL,
    end_date   TEXT NOT NULL,
    caption    TEXT NOT NULL,
    images     TEXT NOT NULL
);
```

字段说明：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `id` | INTEGER | 自增主键 |
| `start_date` | TEXT | 计划开始日期，对应接口字段 `start` |
| `end_date` | TEXT | 计划结束日期，对应接口字段 `end` |
| `caption` | TEXT | 标题，对应接口字段 `caption` |
| `images` | TEXT | 原始图片摘要数组的 JSON 字符串 |

### `images`

保存图片摘要、处理状态和备注。

```sql
CREATE TABLE IF NOT EXISTS images (
    sha256  TEXT PRIMARY KEY,
    status  TEXT NOT NULL CHECK (status IN ('pending', 'processing', 'ready', 'failed')),
    remark  TEXT NOT NULL DEFAULT ''
);
```

字段说明：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `sha256` | TEXT | 原始上传图片内容摘要，也是原图和显示 BMP 的文件键 |
| `status` | TEXT | 图片处理状态：`pending`、`processing`、`ready`、`failed` |
| `remark` | TEXT | 图片备注，用于管理台搜索和人工识别 |

原始图片和显示 BMP 都保存在文件系统中。数据库保存摘要、处理状态和备注，不保存图片二进制内容。

## 读取规则

### 设备读取计划

设备调用 `GET /api/plans`。服务端返回从当前日期开始的计划，默认三天，最多七天。返回给设备的 `images` 使用 `status = 'ready'` 的图片 `sha256`。

如果计划关联的图片仍在处理中，服务端在设备响应中跳过该图片。某条计划下全部图片都未处理完成时，该计划可以保留在响应中并返回空 `images`，设备端按本地缓存和异常策略处理。

查询时使用 `json_each(plans.images)` 展开计划内的图片摘要，并通过 `images.sha256` 关联状态和备注：

```sql
SELECT
    p.id,
    p.start_date,
    p.end_date,
    p.caption,
    image_item.key AS image_index,
    i.sha256,
    i.status,
    i.remark
FROM plans AS p
JOIN json_each(p.images) AS image_item
LEFT JOIN images AS i ON i.sha256 = image_item.value
WHERE p.start_date <= :end_date
  AND p.end_date >= :start_date
ORDER BY p.start_date ASC, p.id ASC, image_item.key ASC;
```

设备响应只收集 `status = 'ready'` 的图片。

用户权限查询在关联图片后增加 `i.status = 'ready'` 过滤。管理员权限不加该过滤，保留计划中的全部图片并返回图片状态。

### 管理台读取计划

管理台调用 `GET /api/plans`，并使用管理员权限。该接口返回每张图片的 `sha256`、`status` 和 `remark`。管理台按 `status` 显示“处理中”“处理失败”或最终图片。

## 前端管理台

管理台位于 `server/web`，使用 Vue 3、Vite、TypeScript 和 bun。页面是工作台式布局，主要功能包括：

- 管理员账号密码登录，并保存管理员 token。
- 图片管理：上传原始图片、填写备注、查看处理状态、搜索图片、预览显示 BMP；`failed` 图片可以通过重新上传同一图片触发重试。
- 计划管理：按天数查看计划，默认三天，最多七天。
- 计划管理：新增、编辑、删除计划，并从已有图片中选择计划图片。

前端开发代理在 `server/web/vite.config.ts` 中配置，开发模式下 `/api` 和 `/images` 转发到 `http://localhost:3000`。

## 工程构建与部署

服务端工程借鉴 `provider-relay` 的独立服务结构，所有后端、管理台和部署文件都收敛在 `server/` 目录中：

```text
server/
  build.rs
  Cargo.toml
  Dockerfile
  docker-build.sh
  docker/docker-compose.yml
  assets/fonts.example.toml
  assets/fonts/
  src/
  tests/
  web/
```

`server/build.rs` 负责在 Cargo 构建服务端时自动编译管理台。默认流程为：

- 监听 `server/web/src`、`index.html`、`package.json`、`bun.lock`、`tsconfig` 和 `vite.config.ts`。
- 如果 `server/web/node_modules` 不存在，执行 `bun install`。
- 执行 `bun run build`，把管理台产物输出到 `server/web/dist`。

构建后端依赖时可以设置 `SKIP_FRONTEND_BUILD=1` 跳过管理台编译，避免 Docker 的 Rust 依赖缓存阶段重复构建前端。

Docker 镜像采用多阶段构建：

- `oven/bun` 阶段安装前端依赖并执行 `bun run build`。
- `cargo-chef` 阶段缓存 Rust 依赖。
- Rust release 阶段设置 `SKIP_FRONTEND_BUILD=1` 编译后端二进制。
- runtime 阶段只拷贝 `epaper-album-server` 二进制和 `web/dist`，运行目录为 `/app`。

容器运行时使用 `/app/data` 作为持久数据目录，保存 SQLite 数据库、原图、显示 BMP 和 sprite 缓存。`server/docker/docker-compose.yml` 提供基础部署配置，服务名和镜像名均为 `epaper-album-server`，部署时通过 `server/.env` 设置 `SECRET_KEY`、`ADMIN_USERNAME` 和 `ADMIN_PASSWORD`。

## 建议验证

服务端实现调整后建议使用以下命令验证：

```powershell
cd server
cargo build
cargo fmt --all
cargo test --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings

cd web
bun run build
```
