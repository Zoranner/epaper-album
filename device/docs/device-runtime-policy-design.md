# 设备运行策略设计文档

## 目标

设备每次启动都进入完整业务周期。唤醒机制只负责让固件按时运行，业务周期统一完成电源检测、同步判断、显示判断和下一次运行安排。

最终目标：

- 启动、复位、烧录后启动和 RTC timer 唤醒都执行同一条业务周期。
- 自动业务周期固定对齐到北京时间下一个整点。
- 外部供电状态下，每个整点同步照片计划和显示资源。
- 电池状态下，每个整点运行一次；当天未成功同步照片计划时继续尝试同步。
- 有效低电状态停止云端同步，并刷新低电状态页。
- 外部供电恢复后，同步最新照片计划，按正常照片页规则刷新屏幕。
- 屏幕显示状态只保存在 `/sdcard/data/state.json`。
- 照片计划同步成功日期只保存在 `/sdcard/data/sync.json`。

本设计把四件事分开：入口分流、整点运行、照片同步、屏幕显示。电源状态只参与业务决策，流程统一保持一套整点周期。

## 总体模型

最终模型：

```text
入口分流 + 整点运行 + 同步决策 + 显示决策
```

职责划分：

| 职责 | 负责内容 |
| --- | --- |
| 入口分流 | 根据自检请求标记决定进入硬件自检还是正常业务周期 |
| 整点运行 | 计算下一次北京时间整点，并选择等待重启或 deep sleep |
| 同步决策 | 判断本轮是否联网同步照片计划和资源 |
| 显示决策 | 判断本轮是否刷新屏幕，以及刷新成什么内容 |

电源状态只影响：

- 本轮是否允许联网同步。
- 本轮是否刷新低电状态页。
- 业务周期结束后使用外部供电等待重启，还是电池 deep sleep。

## 入口分流

固件启动后的第一层入口只处理自检请求。

```text
固件启动
读取 wake reason
读取自检请求标记

存在自检请求:
    进入硬件自检

不存在自检请求:
    把 wake reason 映射为 RunTrigger
    进入完整业务周期
```

冷启动、复位、烧录后启动、未知唤醒和 RTC timer 唤醒都进入完整业务周期。

硬件自检独立于照片业务周期。自检负责读取 TF 卡、解析配置、测试 Wi-Fi 和 HTTP、刷新自检页、写入串口报告。自检页显示后监听 KEY 单击，单击后清除自检请求标记并重启回正常业务周期。

## 按键

按键业务只在外部供电且屏幕已有内容时定义。

| 按键 | 语义 |
| --- | --- |
| `PWR` | 设备开机和关机 |
| `BOOT` | 保留硬件和烧录语义 |
| `KEY` | 相册业务自检 |

用户可见行为：

| 屏幕状态 | KEY 操作 | 行为 |
| --- | --- | --- |
| 相册页 | 长按 5 秒 | 写入自检请求标记，调用 `esp_restart()`，启动后进入自检 |
| 错误页 | 长按 5 秒 | 写入自检请求标记，调用 `esp_restart()`，启动后进入自检 |
| 自检页 | 单击 | 清除自检请求标记，调用 `esp_restart()`，启动后回到正常业务周期 |

KEY 交互覆盖外部供电且已有屏幕内容的状态。KEY 进入自检不依赖开机窗口；设备处于外部供电并显示相册页或错误页后，固件持续监听 KEY 长按。

## 运行节拍

自动运行时间统一为北京时间下一个整点。

| 当前供电分类 | 本轮结束方式 |
| --- | --- |
| 外部供电 | 启动一次 esp timer，到点调用 `esp_restart()` |
| 电池 | 配置 RTC timer，进入 deep sleep |
| 有效低电 | 配置 RTC timer，进入 deep sleep |

deep sleep 唤醒视为一次正常启动。外部供电等待重启也视为一次正常启动。两者进入同一条业务周期，区别只在等待期间的硬件状态和功耗。

## 电源策略

每次业务周期都读取 PMIC。业务层使用两个判断：

```text
external_powered = PMIC 明确存在 VBUS 或充电输入
effective_low_battery = reliable_low_battery && !external_powered
```

规则：

- 外部供电时，有效低电恒为 false。
- PMIC 明确检测到外部供电存在时归入外部供电。
- PMIC 百分比在完成硬件校准前只进入日志和诊断。
- 低电判断默认只接受可靠硬件低电标志。
- 电压阈值经过实测后再纳入有效低电判断。

`PowerProfile` 只表达业务供电分类：

| 分类 | 含义 | 结束方式 |
| --- | --- | --- |
| `External` | PMIC 明确检测到外部供电 | 等待整点重启 |
| `Battery` | 未检测到外部供电，且无有效低电 | deep sleep 到整点 |
| `LowBattery` | 未检测到外部供电，且存在有效低电 | deep sleep 到整点 |

## 照片同步

照片同步负责获取云端照片计划、下载显示图片、下载 caption/date sprite。

同步成功的事实写入 `/sdcard/data/sync.json`。同步失败不更新 `sync.json`，下一个整点继续按规则判断。

同步决策：

```text
如果 effective_low_battery:
    Skip, cause = LowBattery

否则如果 config 缺失或不完整:
    Skip, cause = MissingConfig

否则如果 external_powered:
    Fetch, cause = External

否则如果 sync.date != today:
    Fetch, cause = Daily

否则:
    Skip, cause = Done
```

同步成功后：

- 覆盖写入 `plan.json`。
- 更新 `sync.json.date = today`。
- 用最新照片计划进入显示决策。

同步失败后：

- 保留本地 `plan.json`。
- 保留 `sync.json` 原值。
- 失败原因进入日志和本轮运行报告。
- 继续进入显示决策，优先使用本地可显示内容。

缓存文件存在但不可解析、尺寸不符合或格式不可显示时，视为资源缺失；本轮允许联网同步时重新下载。

## 屏幕显示

正常照片页由四个字段决定：

```text
date + image + caption
```

字段含义：

| 字段 | 含义 |
| --- | --- |
| `date` | 屏幕右下角显示的当前北京时间日期 |
| `image` | 当前屏幕照片资源 `sha256` |
| `caption` | 当前屏幕左下角标题 |

显示决策只比较目标显示和 `state.json`。照片计划日期只用于选择照片，不作为屏幕右下角日期。

刷新规则：

- 目标四字段与 `state.json` 一致时保留屏幕。
- `date`、`image`、`caption` 任一字段变化时刷新。
- 有效低电时刷新低电全屏错误页。
- 有效低电但没有当前可显示照片时，刷新低电状态页。
- 外部供电恢复后按正常照片计划判断是否刷新。
- 同步失败时刷新同步错误状态页。
- 配置缺失、无可用照片时通过显示决策刷新对应状态页。TF 卡挂载失败时由平台层刷新存储错误页。

状态页用于无法生成正常照片页的情况。业务周期内可以访问 TF 卡时，状态页刷新成功后写入 `date` 并清空照片字段，让后续恢复判断有依据。TF 卡挂载失败时只能刷新存储错误页，不能写入 `state.json`。

## 持久状态

`state.json` 只表达屏幕当前实际显示内容。它不保存电源状态、同步结果和失败原因。

```json
{
  "date": "2026-06-13",
  "image": "display-image-sha256",
  "caption": "照片标题",
}
```

字段：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `date` | `LocalDate?` | 屏幕右下角实际显示的当前北京时间日期 |
| `image` | `String?` | 当前屏幕照片资源 `sha256` |
| `caption` | `String?` | 当前屏幕左下角标题 |


`sync.json` 只表达最近一次照片计划成功同步发生在哪一天。

```json
{
  "date": "2026-06-13"
}
```

字段：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `date` | `LocalDate?` | 最近一次照片计划成功同步发生的北京时间日期 |

`sync.json` 缺失或不可解析时，按当天未同步处理。

## 诊断日志

诊断日志用于后期排查设备运行过程，不参与运行策略决策。日志写入失败不能影响同步、显示刷新、等待重启或 deep sleep。

日志文件按天追加写入：

```text
/sdcard/data/logs/YYYY-MM-DD.jsonl
```

每行是一条结构化事件：

```json
{"time":1781373609,"run":1781373600,"level":"info","event":"sync","message":"sync decision resolved","data":{"action":"Fetch","cause":"External","attempted":true,"succeeded":true}}
```

字段：

| 字段 | 含义 |
| --- | --- |
| `time` | 事件发生时的 Unix 秒 |
| `run` | 本轮启动标识，使用本轮进入设备周期时的 Unix 秒 |
| `level` | `info`、`warn` 或 `error` |
| `event` | 事件类型 |
| `message` | 简短可读描述 |
| `data` | 事件上下文 |

第一版记录关键业务链路事件：

| 事件 | 内容 |
| --- | --- |
| `trigger` | 本轮启动触发来源 |
| `time` | SNTP 校时后的 Unix 时间和日期 |
| `power` | 电源档位、电池状态、百分比和低电标记 |
| `cycle` | 设备周期结果 |
| `sync` | 同步决策、是否尝试、是否成功和错误 |
| `display` | 显示决策、刷新目标和刷新结果 |
| `next` | 下一次运行时间和等待方式 |
| `state` | 状态文件写入失败 |

日志只记录诊断事件，不记录 bootloader 全量日志、Wi-Fi 驱动噪声、图片内容、BMP 内容或高频循环日志。TF 卡日志保留最近 14 天，超过窗口的旧日志在后续运行时清理。

## 决策结构

决策结构分成三类：本轮事实、同步决策、显示决策。

```rust
pub struct RunContext {
    pub now: u64,
    pub date: LocalDate,
    pub trigger: RunTrigger,
    pub battery: BatteryStatus,
    pub power: PowerProfile,
    pub config: Option<Config>,
    pub plans: Option<Vec<Plan>>,
    pub state: PersistentDeviceState,
    pub sync: PersistentSyncState,
}
```

字段含义：

| 字段 | 含义 |
| --- | --- |
| `now` | 当前 Unix 秒 |
| `date` | 当前北京时间日期 |
| `trigger` | 本轮启动来源 |
| `battery` | PMIC 原始电源读数 |
| `power` | 业务供电分类 |
| `config` | 可用配置；缺失或不完整时为 `None` |
| `plans` | 本地或同步后的照片计划 |
| `state` | 当前屏幕显示状态 |
| `sync` | 最近一次照片计划成功同步状态 |

```rust
pub struct SyncDecision {
    pub action: SyncAction,
    pub cause: SyncCause,
}

pub enum SyncAction {
    Fetch,
    Skip,
}

pub enum SyncCause {
    External,
    Daily,
    Done,
    LowBattery,
    MissingConfig,
}
```

`SyncDecision` 只回答本轮是否同步照片计划和资源。

```rust
pub struct DisplayDecision {
    pub action: DisplayAction,
    pub cause: DisplayCause,
}

pub enum DisplayAction {
    Keep,
    Refresh(DisplayTarget),
}

pub enum DisplayTarget {
    Photo {
        date: LocalDate,
        image: String,
        caption: String,
    },
    Page {
        date: LocalDate,
        title: String,
        message: String,
        detail: String,
    },
}

pub enum DisplayCause {
    First,
    Date,
    Photo,
    Recovery,
    Sync,
    MissingConfig,
    MissingPhoto,
    Same,
}
```

`DisplayDecision` 只回答屏幕是否刷新，以及刷新成什么。`DisplayTarget::Photo` 使用屏幕四字段，不接收 `Plan`。

`cause` 只用于日志和测试，不写入持久状态。

## 业务流程

```text
业务周期开始
读取 PMIC
读取时间
读取配置
读取 plan.json
读取 state.json
读取 sync.json
生成 RunContext

执行 decide_sync(context)

如果 sync.action == Fetch:
    连接 Wi-Fi 并完成 SNTP 校时
    用校时后的时间重新生成 now/date
    同步照片计划和资源
    成功后写 plan.json 和 sync.json
    失败后记录日志和本轮运行报告

用最新事实执行 decide_display(context)

如果 display.action == Refresh(target):
    刷新照片页或状态页
    TF 卡可写时成功后写 state.json

计算北京时间下一个整点
根据供电分类进入等待重启或 deep sleep
```

## 显示决策规则

```text
如果配置缺失:
    Refresh(Page(MissingConfig))

否则如果 effective_low_battery:
    Refresh(Page(date=today, title=LOW BATTERY))

否则如果 sync failed 且 selected 可显示:
    Refresh(Page(date=today, title=SYNC ERROR))

否则如果 sync failed 且 state.image 可显示:
    Refresh(Page(date=today, title=SYNC ERROR))

否则如果 selected 可显示且目标四字段与 state 一致:
    Keep

否则如果 selected 可显示:
    Refresh(Photo(date=today, image=selected.image, caption=selected.caption))

否则:
    Refresh(Page(MissingPhoto))
```

`selected` 表示按当前北京时间日期从照片计划中选出的照片。

## 关键场景

| 场景 | 预期行为 |
| --- | --- |
| 烧录后启动 | 进入完整业务周期，按当前电源、配置、本地状态和同步状态执行 |
| 冷启动或复位 | 进入完整业务周期，记录启动来源并执行业务判断 |
| RTC 整点唤醒 | 进入完整业务周期，检测电源、判断同步、判断显示 |
| 外部供电相册页长按 KEY 5 秒 | 写入自检请求标记，重启进入硬件自检 |
| 外部供电错误页长按 KEY 5 秒 | 写入自检请求标记，重启进入硬件自检 |
| 自检页单击 KEY | 清除自检请求标记，重启回正常业务周期 |
| 外部供电运行 | 每个北京时间整点同步计划，必要时刷新屏幕 |
| 外部供电后转为电池 | 下一个整点识别为电池；当天已同步成功时不重复联网 |
| 电池运行 | 每个整点唤醒检测；当天未成功同步时尝试同步 |
| 电池同步失败 | 本轮记录失败，下个整点继续尝试 |
| 电池有效低电 | 停止云端同步，刷新低电状态页 |
| 外部供电恢复 | 下一个整点同步最新计划，按正常照片页规则刷新屏幕 |
| 图片缓存坏文件 | 视为资源缺失；允许联网时重新下载 |
| 同步失败但有可显示照片 | 刷新同步错误状态页 |
| 配置缺失 | 显示配置状态页 |
| TF 卡不可用 | 平台层显示存储错误页 |

## 模块落点

| 模块 | 职责 |
| --- | --- |
| `device/src/power/mod.rs` | 供电分类、有效低电判断、北京时间下一个整点计算 |
| `device/src/domain/state.rs` | `PersistentDeviceState` 和 `PersistentSyncState` |
| `device/src/storage` | `plan.json`、`state.json`、`sync.json` 读写 |
| `device/src/app/cycle.rs` | `RunContext`、`decide_sync`、`decide_display` 和业务周期编排 |
| `device/src/platform/espidf.rs` | 采集硬件事实、挂载存储、调用业务周期、安排下一次运行 |
| `device/src/main.rs` | 入口分流、运行报告日志、最终进入等待重启或 deep sleep |
| `device/src/selftest` | 硬件自检流程、自检页、自检退出 |
| `device/src/platform/button.rs` | KEY 长按、KEY 单击、自检请求标记 |

## 测试清单

纯函数测试：

- 下一个运行时间始终是北京时间下一个整点。
- 冷启动、烧录后启动、RTC timer 唤醒都会进入完整业务周期。
- 外部供电相册页 KEY 长按 5 秒进入硬件自检。
- 外部供电错误页 KEY 长按 5 秒进入硬件自检。
- 自检页 KEY 单击退出自检。
- 外部供电状态返回 `External`。
- 外部供电时 PMIC 低电标志不产生有效低电。
- PMIC 百分比为 0 时默认不触发低电。
- 电池状态且 `sync.date != today` 时请求同步。
- 电池状态且 `sync.date == today` 时跳过同步。
- 电池同步失败后 `sync.date` 保持旧值。
- 电池同步失败后下个整点继续请求同步。
- 低电时刷新低电全屏错误页。
- 低电且无当前照片时刷新低电状态页。
- 外部供电恢复时按正常照片计划恢复。
- 同步失败时刷新同步错误全屏页。
- 图片缓存存在但不可渲染时触发重新下载。
- `state.date` 表示屏幕右下角当前日期。
- `sync.json` 缺失时按当天未同步处理。

构建验证：

```powershell
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features

$env:IDF_TOOLS_PATH='C:\Espressif'
. .\scripts\activate-esp-idf.ps1
cargo +esp build --target xtensa-esp32s3-espidf
```

设备验证：

- 外部供电整点同步计划成功。
- 外部供电整点无变化时不刷新屏幕。
- 外部供电相册页长按 KEY 5 秒进入自检。
- 自检页单击 KEY 回到正常业务周期。
- 转为电池后下一个整点日志显示电池状态。
- 电池当天已同步成功后整点不重复联网。
- 电池当天未同步成功时整点继续尝试。
- 低电状态刷新低电状态页。
- 外部供电恢复后按正常照片页规则刷新。
- 串口日志记录 PMIC、power profile、sync decision、display decision、sync result、refresh result、next run。

## 迁移步骤

1. 调整 `PowerProfile` 为 `External`、`Battery`、`LowBattery`，统一下一次运行时间为北京时间下一个整点。
2. 调整 `PersistentDeviceState`，让 `date` 表示屏幕右下角当前日期。
3. 增加 `PersistentSyncState` 和 `sync.json` 读写。
4. 调整显示请求，让 date sprite 使用本轮当前日期。
5. 在 `cycle` 中实现 `RunContext`、`SyncDecision`、`DisplayDecision` 纯函数。
6. 用同步决策替换当前固定同步逻辑。
7. 用显示决策替换当前分散的低电、恢复和同步失败刷新逻辑。
8. 调整 ESP 结束方式：外部供电等待整点重启，电池和有效低电 deep sleep 到整点。
9. 增加自检请求标记、KEY 长按 5 秒进入自检、KEY 单击退出自检。
10. 增加串口日志，覆盖 wake reason、trigger、PMIC、power、sync、display 和 next run。
11. 完成主机测试、Clippy、ESP 构建和设备验证。

## 风险控制

deep sleep 只由电池侧结束方式触发。外部供电状态使用整点重启等待，保证设备在有供电时保留 USB/JTAG 可恢复能力。

低电策略只使用可靠硬件低电标志。PMIC 百分比和电压阈值先进入日志，完成实测后再进入有效低电判断。

电池模式每天同步由 `sync.json.date` 表达成功事实。同步失败保留旧日期，让下一个整点继续尝试。

屏幕刷新由目标显示和 `state.json` 对比决定。低电、同步失败和缺图都使用全屏错误页，照片页只显示照片、标题和日期。

## 结论

设备最终运行模型是：启动后先做入口分流；存在自检请求时进入硬件自检；其他启动来源都进入完整业务周期。业务周期采集硬件和本地状态，先做同步决策，再做显示决策，最后安排北京时间下一个整点运行。

外部供电状态每个整点重启运行并允许同步；电池状态每个整点 deep sleep 唤醒，当天未成功同步时继续尝试；有效低电状态停止同步并显示低电状态页。显示正确性由 `state.json` 三个屏幕字段保证，同步可靠性由 `sync.json.date` 保证。
