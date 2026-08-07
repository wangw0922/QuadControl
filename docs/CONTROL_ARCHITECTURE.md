# 控制架构决策

这份文档是四端控制方向的**唯一权威依据**。原始计划
`QuadControl_四端通用控制软件制作计划.md` 中与本文冲突的部分以本文为准。

修订日期：2026-08-06。

## 四象限

| 控制端 | 被控端 | 方案 | QuadControl 负责什么 |
|---|---|---|---|
| Windows | Android | 自研（ADB / scrcpy 式） | 全部：画面、输入、文件、熄屏 |
| macOS | Android | 自研（ADB / scrcpy 式） | 全部：画面、输入、文件、熄屏 |
| Windows | iPhone | go-ios + WebDriverAgent | 客户端与桌面 GUI |
| macOS | iPhone | **Apple iPhone Mirroring** | **不自研**，只做可用性检测与引导 |

前三格是产品要交付的代码。第四格不是。

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
使用前提。**不得**尝试驱动、注入或自动化它的界面 —— 那既无公共接口，也违反本
项目不使用私有 API 的约束。

以下前提来自 Apple 的公开说明，**尚未在本项目中实测**，接入前需要用真机确认：
需要同一 Apple 账户、两台设备靠近、Wi-Fi 与蓝牙开启、iPhone 处于锁定状态。
其中"iPhone 保持锁定且屏幕不亮"这一条如果成立，意味着 Mac→iPhone 这一格
天然满足了熄屏控制需求，而这是我们自研方案从未做到过的。

## 为什么 Windows→iPhone 走 go-ios / WebDriverAgent

Windows 上没有 iPhone Mirroring，Apple 也没有等价方案，因此这一格必须自研。
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

go-ios 的具体能力清单来自其项目说明，**本项目尚未实测**。接入前的第一步是在
真机上验证截图与点击，把清单变成实测事实，而不是转述。

## 被移出范围的方向

以下方向此前在计划中，现在明确移除，不再投入：

- **蓝牙 HID（Mac 侧与 Windows 侧）。** 移除理由是**失去用途**，不是被证伪。
  Mac→iPhone 已由 Apple 方案覆盖，Windows→iPhone 由 WDA 覆盖，HID 不再服务于
  任何象限；而 Windows 侧本就需要另写一套与 Mac 完全独立的实现。
  实测状态如实记录在 `MACOS_HID_FEASIBILITY.md`：本地 API 部分**可用**（SDP
  记录能发布、两个 PSM 都能注册），手机侧三项关键未知（Class of Device 能否
  修改、能否进入可发现状态、iPhone 会不会配对）从未验证。若将来 Apple 方案与
  WDA 两条路都走不通，HID 仍是未被否定的候选。
- **iOS ReplayKit Broadcast Upload Extension。** Windows→iPhone 用 WDA 的画面
  通道，Mac→iPhone 用 Apple 方案，两边都不需要我们自己的广播扩展。
- **iOS Screen Curtain 实验。** 熄屏控制在 Mac→iPhone 一格由 Apple 方案覆盖，
  在 Windows→iPhone 一格确定不可达。没有中间地带需要实验。
- **自研 Mac↔iPhone 连接协议。** 仓库中现存的 Apple 诊断栈（`QuadControlDiagnostic*`、
  `QuadControlMacListener`、`apps/ios`）是为验证这条链路而建的，现已不在产品
  路径上。它仍是仓库里唯一经过真机端到端验证的组件，暂时保留为连接诊断工具，
  但不得再作为产品能力宣传。

## 熄屏能力的真实分布

计划最初要求"手机实体屏幕关闭后，电脑继续显示和操作"。在新架构下这条的达成
情况是不对称的，必须如实标注：

| 象限 | 熄屏控制 |
|---|---|
| Windows → Android | 可达（自研，ADB 路径） |
| macOS → Android | 可达（自研，ADB 路径） |
| macOS → iPhone | 由 Apple 方案覆盖（待实测确认） |
| Windows → iPhone | **不可达**，WDA 机制决定 |

## 不变的约束

方向调整不放松任何安全边界：不使用私有 API、不越狱、不 Root、不绕过锁屏、
不隐藏控制、不默认无人值守。所有连接都必须由用户主动发起并可见、可断开。
