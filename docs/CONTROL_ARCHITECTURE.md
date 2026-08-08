# 控制架构决策

这份文档是控制方向的**唯一权威依据**。原始计划
`QuadControl_四端通用控制软件制作计划.md` 中与本文冲突的部分以本文为准。

修订日期：2026-08-08（新增 Linux 控制端；Android 引擎改为采用 scrcpy）。

## 控制矩阵（3 控制端 × 2 被控端）

| 控制端 | 被控端 | 引擎 | QuadControl 负责什么 |
|---|---|---|---|
| Windows / macOS / Linux | Android | **scrcpy**（外部引擎，Apache-2.0） | 封装：发现、配对、启动配置、退出清理、统一 GUI |
| Windows / Linux | iPhone | go-ios + WebDriverAgent | 客户端与桌面 GUI |
| macOS | iPhone | **Apple iPhone Mirroring** | **不自研**，只做可用性检测与引导 |

矩阵能压缩成三行，是因为两个引擎本身就是跨平台的：scrcpy 官方支持
Windows/macOS/Linux，go-ios（Go 编写）同样三平台可用。我们交付的是统一的
控制端软件与 iPhone 侧集成，不是重写引擎。

已实测：macOS→Android（scrcpy 4.1，见下）；Windows/Linux→iPhone 的 WDA 链路在
iPhone SE 3 上验证过（经 macOS 宿主）。**未实测**：Windows/Linux 上运行
scrcpy、Linux 上运行 go-ios ——文档如实标注，接入前逐项真机验证。

## 为什么 Mac→iPhone 不自研

Apple 在 macOS 15 / iOS 18 起提供 iPhone Mirroring，官方实现了投屏与控制，
并且是唯一被 Apple 允许的形态。自研只会得到一个更差、更脆弱、且无法上架的
替代品。这一格的正确做法是把用户导向官方方案。

本机核实（macOS 26.5.1，Apple M1 Pro）：

- 应用存在于 `/System/Applications/iPhone Mirroring.app`
- bundle identifier 为 `com.apple.ScreenContinuity`
- `LSMinimumSystemVersion` 为 `26.5`
- **没有 URL scheme，没有公共 API**

因此 QuadControl 对这一格能做的仅限于：检测该应用是否存在、检测系统版本是否
满足、用 `open -b com.apple.ScreenContinuity` 调起它、以及在文档和界面里说明
使用前提。**不得**尝试驱动、注入或自动化它的界面 —— 那既无公共接口，也违反
本项目「除已明确约束的 Android scrcpy 例外外，不使用私有 API」的约束。

以下前提来自 Apple 的公开说明，**尚未在本项目中实测**，接入前需要用真机确认：
需要同一 Apple 账户、两台设备靠近、Wi-Fi 与蓝牙开启、iPhone 处于锁定状态。
其中"iPhone 保持锁定且屏幕不亮"这一条如果成立，意味着 Mac→iPhone 这一格
天然满足了熄屏控制需求，而这是我们自研方案从未做到过的。

## Android 引擎：采用 scrcpy（2026-08-08 修订）

**决策：Android 被控端的画面与输入引擎直接采用 scrcpy，不再自研。**
用户 2026-08-08 拍板："直接使用它的也可以，不要造过多的轮子。"

证据链，按时间顺序如实记录：

1. **2026-08-07 真机勘探**发现三星 SM-S9180（Android 16）上
   `SurfaceControl.createDisplay` 与 `setDisplaySurface` 不存在，AOSP 的
   `ScreenCapture` 嵌套类全部缺失，被 `Sem` 前缀的三星定制接口取代。当时的
   结论是"照抄 scrcpy 要为每个 OEM 重做勘探"，因此转向官方命令打底的两层方案。
2. **2026-08-08 官方路径到达延迟地板**：`screenrecord` 路径实测「触发→字节到达
   Mac」= 0.56 秒（其中 ≥0.26 秒在字节离开设备之前），码率、分辨率、帧率任何
   参数都无法改善。带宽被排除（健康 Wi-Fi 链路 33–43 Mbps，8M 流只占 21%，零
   丢包）。
3. **2026-08-08 scrcpy 4.1 在同一台三星上直接跑通**，用户确认"延迟消失"。
   这推翻了第 1 条的推论：per-OEM 适配由 scrcpy 社区承担并持续维护——这正是
   不造轮子的理由。未测试的 OEM 上它仍可能失效，风险表述与
   PRODUCT_CONSTRAINTS.md 保持一致。

scrcpy 之所以低延迟，每项技术都对着官方路径的一块地板（也是我们采纳它的
技术清单）：自建 server 经 `app_process` 运行、隐藏 API 建镜像面直喂
`MediaCodec.createInputSurface()`（绕开 screenrecord 内部缓冲）、编码器
realtime 优先级 + 无 B 帧 + `KEY_MAX_FPS_TO_ENCODER` 限帧（**不改设备刷新率**，
这是用户红线）、localabstract socket + `adb forward`（绕开 adbd shell 流缓冲）、
客户端只显示最新帧。

顺带获得的能力（原两层计划中的未竟项）：UTF-8 文本注入（中文输入）、音频转发
（Android 11+）、`--turn-screen-off` 熄屏控制、真连续拖拽手势。

**原第一层（官方命令路径）降级为回退方案**：scrcpy 不可用（未安装、设备策略
拦截）时仍能出画面和基本控制。它已实现并保留在
`apple/Sources/QuadControlAndroidVideo/`，实测记录如下，不再继续投入：

**第一层：官方命令打底。** `adb exec-out screenrecord --output-format=h264
--time-limit 0 -` 与 `adb shell input`。全部是官方 shell 工具，不含任何
内部 API，预期兼容面比依赖隐藏 API 的路径更宽（实测样本仍只有一台设备）。

本机实测（三星 SM-S9180 / Android 16）：

| 项 | 实测值 |
|---|---|
| H.264 流 | 1280×596 @ 25 fps，ffprobe 确认有效 |
| 首字节延迟 | 0.37 秒 |
| 流式性 | 持续输出约 500 KB/s，非录完才给 |
| 时长限制 | `--time-limit 0` 官方支持无限 |
| 单张截图 | 3088×1440，1.42 秒/张 |

代价如实记录：`input` 每次事件要起一个进程，比直接注入慢；**熄屏后继续操作
这一层做不到**。

原"第二层：自研内部 API 增强"由 scrcpy 采纳决策取代，不再实施。
`agents/android-shell/server` 的能力探测器保留：它当初发现的三星 API 偏离
正是本次决策的证据之一，且未来排查某台设备上 scrcpy 失效时仍是勘探工具。

## 为什么 Windows/Linux→iPhone 走 go-ios / WebDriverAgent

Windows 和 Linux 上没有 iPhone Mirroring，Apple 也没有等价方案，因此这两格
必须自研。
go-ios（MIT）已经解决了 usbmuxd、设备隧道、WDA 安装与运行这些最脏的部分，且
本身就是跨平台的。

必须如实告知用户的限制：

- **这是 UI 自动化，不是输入注入。** 普通应用的点击、文字输入、设置操作可用；
  游戏、连续拖拽、多指手势和高帧率场景的体验达不到 scrcpy 的水平。这是机制
  决定的，不是优化能解决的。
- **配置门槛高**：信任电脑、开启开发者模式、开启 UI Automation、用有效证书
  签名并安装 WDA、iOS 17+ 需要设备隧道、Windows 需要隧道驱动。
- **签名是主要门槛**：免费个人账号签出的 WDA 七天过期，付费开发者账号才顺畅。
- **无法上架分发**：WDA 是开发者测试工具。这一格只能作为开发者/技术用户自用的
  能力，不能作为面向普通消费者的商店产品。
- **不支持熄屏控制**：屏幕关闭或锁定时 XCUITest 无法驱动界面，也取不到画面。

go-ios 的具体能力与 iPhone SE 3 的实测记录见 [STATUS.md](STATUS.md)。

## 被移出范围的方向

以下方向此前在计划中，现在明确移除，不再投入：

- **蓝牙 HID（Mac 侧与 Windows 侧）。** 移除理由是**失去用途**，不是被证伪。
  Mac→iPhone 已由 Apple 方案覆盖，Windows/Linux→iPhone 由 WDA 覆盖，HID 不再服务于
  任何象限；而 Windows 侧本就需要另写一套与 Mac 完全独立的实现。
  实测状态如实记录在 `MACOS_HID_FEASIBILITY.md`：本地 API 部分**可用**（SDP
  记录能发布、两个 PSM 都能注册），手机侧三项关键未知（Class of Device 能否
  修改、能否进入可发现状态、iPhone 会不会配对）从未验证。若将来 Apple 方案与
  WDA 两条路都走不通，HID 仍是未被否定的候选。
- **iOS ReplayKit Broadcast Upload Extension。** Windows/Linux→iPhone 用 WDA 的画面
  通道，Mac→iPhone 用 Apple 方案，两边都不需要我们自己的广播扩展。
- **iOS Screen Curtain 实验。** 熄屏控制在 Mac→iPhone 一格由 Apple 方案覆盖，
  在 Windows/Linux→iPhone 两格确定不可达。没有中间地带需要实验。
- **自研 Mac↔iPhone 连接协议。** 仓库中现存的 Apple 诊断栈（`QuadControlDiagnostic*`、
  `QuadControlMacListener`、`apps/ios`）是为验证这条链路而建的，现已不在产品
  路径上。它仍是仓库里唯一经过真机端到端验证的组件，暂时保留为连接诊断工具，
  但不得再作为产品能力宣传。

## 熄屏能力的真实分布

计划最初要求"手机实体屏幕关闭后，电脑继续显示和操作"。在新架构下这条的达成
情况是不对称的，必须如实标注：

| 象限 | 熄屏控制 |
|---|---|
| Windows / macOS / Linux → Android | 可达（scrcpy `--turn-screen-off`；macOS 上已实测：屏灭后镜像与控制正常，2026-08-08，SM-S9180） |
| macOS → iPhone | 由 Apple 方案覆盖（待实测确认） |
| Windows / Linux → iPhone | **不可达**，WDA 机制决定 |

## 不变的约束

方向调整不放松任何安全边界：除 Android scrcpy 这一已明确约束的例外外，不使用私有 API；
不越狱、不 Root、不绕过锁屏、
不隐藏控制、不默认无人值守。所有连接都必须由用户主动发起并可见、可断开。
