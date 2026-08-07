# QuadControl Progress

## 2026-08-04

- 已读取完整项目计划（957 行）和 `planning-with-files` skill。
- 已确认项目尚无代码且不是 Git 仓库。
- 已建立任务计划、发现记录和进度日志。
- 独立 `gpt-5.6-sol` 方案评审结论为 `NEEDS-CHANGES`：无 blocking，5 项 major，3 项 minor。
- 已修正 ADB 无线状态语义，补齐目录/文档映射、被动探测安全合同和错误路径测试范围。
- 已盘点工具链：缺少 Rust、ADB、.NET、Gradle 和完整 Xcode；Docker 可用。
- 阶段 1 完成：方案评审问题已纳入验收合同。
- 实现会话已创建 Rust workspace、协议 schema、ADB probe、11 个核心文档、四端说明和验证脚本。
- 主协调器已读取全部新增文件并修正 ADB 连接语义、transport 证据、错误输出解析和 JSON 转义。
- `sh -n scripts/*.sh` 通过；主机验证脚本及三端 shell 构建脚本均明确报告 `SKIP`。
- Rust 验证尝试：Docker Rust 镜像拉取两次停滞；Ubuntu 容器 apt 停滞；主机临时 rustup 速度过慢。均已停止，不将其记为通过。
- 独立代码 vs 方案评审首次发现真实 runner/CLI 测试覆盖不足；主协调器补充 Unix 临时假 adb 集成测试，覆盖真实子进程、超时回收、双流超限、adb 缺失和 CLI `0/1/64`。
- 当前共有 24 项 Rust 单元/集成测试。独立复核确认上一轮唯一 major 的测试覆盖代码已闭合，无新增 major/minor。
- 静态验证通过：59 个任务文件存在，11 个核心文档非空，shell 语法通过，新增文本无尾随空白，执行源码无禁止的 ADB 命令字面量。
- 最终状态：**Blocked**（仅 Rust metadata/fmt/Clippy/tests 未实际运行）。已清理临时 rustup 文件；按用户要求在此暂停，等待重启后继续验证。

## 2026-08-04 重启后

- 已恢复 `task_plan.md`、`findings.md`、`progress.md` 和项目 `AGENTS.md`。
- 已读取 `planning-with-files`、`swiftpm-macos`、`swiftui-ui-patterns`、`ios-debugger-agent` 完整指令。
- 当前正在盘点 Xcode/Simulator/Swift/Rust/ADB 工具状态，并准备追加 macOS/iOS 连接诊断阶段。
- 已读取 SwiftUI `app-wiring` 参考；确定本轮 UI 采用根级依赖注入、单一连接状态与小型子视图，不引入全局单例或复杂路由。
- 工具盘点完成：Swift CLI 可用；完整 Xcode、Simulator、Rust、ADB、XcodeGen/Tuist 不可用。XcodeBuildMCP `list_sims` 明确失败于缺少 `simctl`。
- 已在 `task_plan.md` 追加阶段 6-10，并将本轮范围锁定为非敏感、用户主动的局域网连接诊断闭环。
- Swift 6.3.2 实际 framework 导入检查通过：Foundation、Network、CryptoKit、SwiftUI；SwiftPM CLI 可用。
- 已派发独立 Apple M0 安全/结构方案评审，等待结论后再实现。
- 独立 Apple 方案评审返回 `NEEDS-CHANGES`：2 blocking、5 major、3 minor。
- 已将 schema-first、固定 transcript、双向 proof/session MAC、资源上限、LAN 接口与 token 泄露边界纳入 `task_plan.md`；准备按收紧后的合同委托实现。
- 实现会话已落盘 Swift package/iOS manifest/docs，并报告 `swift build` 成功；主协调器完整读取后拒绝其完成声明，因为网络时限/资源上限/限速/实际 heartbeat 缺失且 XCTest 为零测试。
- 主协调器复跑：`swift package/build` 使用 `--disable-sandbox` 通过；`swift test` 仅构建、没有执行测试。直接导入 XCTest/Testing 均失败，确认需增加 self-test 可执行目标。
- 主协调器已把初稿拆为 protocol、transport、server 三个 Swift target，补齐双向握手、HKDF、session MAC 心跳、资源/限速、私网地址解析及一次性 token 合同。
- 首次重构构建暴露顶层 private 类型与 throwing autoclosure 编译错误；已做最小修复，`swift build --disable-sandbox` 现通过。
- `QuadControlSelfTest` 已补充固定 HMAC/HKDF vectors、错误版本/长度、错误 token、握手超时、一次性消费、序号 replay、资源/限速与真实 Network.framework 回环；实际 39 个断言全部通过。
- macOS listener 的非交互启动 smoke test 正确以 exit 64 拒绝，证明 token 不会在可重定向 stdout 环境中显示；交互终端路径仍需用户本机终端验证。
- 客户端目标地址已收紧为数字 loopback/private/link-local，和 listener 的指定接口/private peer 双边限制一致；重跑 build 与 self-test 后 40 项断言通过。
- iOS Info.plist 已补齐可安装 bundle 基础键，XcodeGen manifest 增加共享 scheme，SwiftUI tests 增加无效 token 清理场景。
- `CONNECT_AND_TEST.md` 已补齐自动回环、XcodeGen/签名/iPhone 同网连接、预期心跳、故障排查与 Android 仅 ADB 就绪探测的可执行步骤。
- 主协调器继续做全实现审读，修正三处语义风险：Swift Codable key 与 `.proto` 对齐；heartbeat MAC 改签 canonical semantic status 而非 JSON bytes；per-peer 限速 key 去除临时源端口，并从连接创建时即启动 10 秒握手超时。重跑后 41 项断言通过。
- 去除默认 token 构造中的 `try!`，终端 token 改为单次可失败写入；iOS 用 attempt ID 忽略旧连接的迟到回调，避免用户 Disconnect 后状态被旧失败覆盖。新增 schema key/type 断言后 self-test 共 45 项通过。
- 根据独立终审初步意见，client 只把规范化后的 `NWEndpoint.Host` 交给连接层，并新增 bracket/scope/hostname/public negatives；heartbeat deadline 现在在 send 前启动；连接中即可 Disconnect；listener ready/token 加 once guard；verify 脚本显式报告 iOS build 状态；协议文档列出每个 transcript。重跑后 49 项断言通过。
- 独立终审指出首个 server heartbeat event 早于 ACK send，原 loopback 可能假阳性；self-test 现必须等到 sequence 2（证明 client 已验证 ack1 后继续），并增加损坏 heartbeat MAC 与跨域 ack MAC negative。实际 51 项断言通过。
- 最终从 `swift package clean` 后运行 `scripts/verify-all.sh`：Swift package describe/build PASS，`QuadControlSelfTest` 51 assertions PASS；Cargo、XCTest、完整 Xcode/XcodeGen 如实 Blocked。`sh -n`、Info.plist lint、project.yml YAML parse、iOS app/test `swiftc -parse` 全部通过；非交互 listener 按合同 exit 64 且未输出 token。
- 独立 `gpt-5.6-sol` 代码 vs 方案终审最终结论 **PASS**，无 blocking、major 或 minor。项目不是 Git 仓库，因此未创建 commit、tag 或 merge；本轮发布收尾以 README/STATUS/CONNECT_AND_TEST 与验证记录为准。

## 2026-08-06 完整 Xcode 环境验证

- 复核主机工具链：完整 Xcode 26.6 + iOS 26.5 SDK + `simctl` + iPhone 17 模拟器已 Booted；`cargo`/`adb` 仍缺。此前的 iOS Blocked 前提已消失。
- `swift test` 首次实际执行 XCTest：4 项通过（历史上 CLT 环境从未跑过断言）。
- 安装 XcodeGen 2.46.0，`xcodegen generate` 成功产出 `QuadControlIOS.xcodeproj`。
- iPhone 17（iOS 26.5）模拟器 `xcodebuild test`：`** TEST SUCCEEDED **`，2 项 `ConnectionModelTests` 通过。
- 用真实 listener 二进制做端到端连接时暴露生产缺陷：loopback + 显式端口触发 `NWListener` EINVAL，默认调用的 listener 从来无法启动。已定位、用四组对照实测确认修复方案并修复。
- 先补回归断言并验证其在未修复代码上 FAIL、修复后 PASS，self-test 51 → 52 项；连跑 3 次稳定通过。
- 模拟器端到端实测通过：handshake、authenticated、heartbeat 1..10 每 5 秒递增、用户 Disconnect、TTL 内 token 复用拒为 `consumed`、超 TTL 复用拒为 `expired`；日志只含短连接 ID。
- 修正文档与实际行为不符处：服务端拒绝不回错误码，iPhone 只显示 `network`，真正原因需看 Mac 日志；该行为记为显式安全决策。
- 清理重构遗留：删除空目录 `QuadControlDiagnosticClient`，`ARCHITECTURE.md` 改为实际 target 名。
- 已更新 STATUS/RISKS/DECISIONS/IOS_FEASIBILITY/CONNECT_AND_TEST/README，iOS 从"源码完成、运行 Blocked"改为"模拟器已验证、真机签名仍未验证"。
- 修复经 PR #2 合并到 `main`（squash `700a9dc`），CI 的 Rust 与 Apple 两项均通过。

## 2026-08-06 真机验证、简体中文与扫码配对

- 用户完成两项人工前置：iPhone 开启 Developer Mode、Xcode 登录 Apple ID。
- 从 Xcode 偏好中取到个人 Team `TEAMIDREDACT`；Team ID 不写入仓库，改由命令行传入，保持"不保存个人签名信息"的既有承诺。
- iPhone SE 3 真机签名构建、安装、启动成功；首次启动需用户手动信任开发者描述文件。
- **真机 Wi-Fi 局域网连接验证通过**：handshake → authenticated → heartbeat 1、2 → 主动断开。补上了模拟器无法证明的 LAN 路由、本地网络权限与真机签名。
- 简体中文本地化：iOS 加 `en`/`zh-Hans` 两套 `.lproj`，status 由硬编码英文串重构为 `ConnectionStatus` 枚举；`InfoPlist.strings` 覆盖本地网络与相机权限弹窗文案。
- macOS listener 的 token 展示段本地化（按用户要求日志行保持英文）；过程中修掉 SwiftPM 小写 `.lproj` 和 CLI 无 app bundle 两个导致中文静默失效的问题。
- 用户提出把 token 改成固定 `123456`。已拒绝，并给出分析：随机 6 位 PIN 同样不可行，因为 client proof 可被离线暴力破解，限速无效。改为实现扫码配对。
- 扫码配对：`session.proto` 先定义 `DiagnosticPairingPayload`，Mac 用 CoreImage 在终端渲染二维码并解析本机私网地址，iOS 加相机扫码并复用私网校验与 token 解码路径。
- **扫码配对真机验证通过**：handshake → authenticated → heartbeat 1-4。
- 测试增至：self-test 62 项断言、iOS 9 项 XCTest（含四种恶意二维码拒绝场景）、Swift package 4 项 XCTest。
- 简体中文与扫码配对经 PR #3 合并到 `main`（squash `07ddc30`），CI 双绿。

## 2026-08-06 M0：macOS 蓝牙 HID 能力检测

- 新增 `QuadControlHIDProbe` 库 + `QuadControlMacHIDProbe` CLI，结构对齐 Rust ADB probe：解析与执行解耦，可无硬件单测。
- 保持被动：只跑 `system_profiler SPBluetoothDataType -json` 一条只读命令（15 秒超时、1 MiB 输出上限），其余仅做 Objective-C 运行时的类/选择器存在性检查，不发布 SDP 记录、不开通道、不配对。
- 本机实测结果：macOS 26.5.1 / BCM_4387 控制器已上电，支持服务含 HID；`IOBluetooth` 可加载；三个外设角色 API（发布 SDP 记录、撤销记录、注册 PSM 0x11/0x13 入站 L2CAP）在 26.5 SDK 中均存在且无弃用标记。
- `can_act_as_hid_peripheral` 恒为 `unknown` 并有测试守护——控制器支持 HID 只证明射频支持该 profile，不等于本进程可担任外设角色。
- 不报告控制器蓝牙地址：稳定硬件标识符，本诊断不需要，与 ADB probe 不持久化设备序列号一致。
- 已输出 `docs/MACOS_HID_FEASIBILITY.md`，写明仍需一次用户在场的主动实验（发布记录 → 让 iPhone 尝试配对 → 发一个 HID report → 撤销记录并还原可发现状态）。
- 测试增至 19 项 Swift package XCTest（4 协议 + 15 HID probe）。
- 经 PR #4 合并到 `main`（squash `a2b3f34`）。首次 CI Apple job 失败是 GitHub 侧 `Failed to resolve action download info` 基础设施故障，重跑后双绿。

## 2026-08-06 HID 外设角色主动实验（第一部分）

- 用户在场，同意做会改动蓝牙状态的主动实验。
- 四组对照确定进程形态要求：必须打成 `.app` bundle 且经 LaunchServices 启动，否则 TCC 直接 SIGABRT；崩溃提示指向 Info.plist 缺 key，具有误导性。
- 自我纠错一次：首轮 SDP 发布全 nil，误以为策略拦截，实为字典格式错误。对照 Apple `OBEXOPPSDPRecord.plist` 修正 UUID 与 ServiceName 编码后成功。
- 已确认：HID service class `0x1124` 可发布并撤销；PSM `0x0011`/`0x0013` 均可注册。macOS 未对第三方保留 HID 角色。
- 未确认：完整 HID 记录（report descriptor 等）、Class of Device 能否改（最可疑的阻塞点）、可发现状态控制、iPhone 是否会列出并配对。
- 实验后蓝牙状态已还原，无残留进程与记录。

## 2026-08-06 控制方向重构

- 用户决定四象限方案：Windows/macOS→Android 自研；Windows→iPhone 走 go-ios + WebDriverAgent；**macOS→iPhone 使用 Apple 官方 iPhone Mirroring，不自研**。
- 本机核实 iPhone Mirroring：存在于 `/System/Applications/`，bundle id `com.apple.ScreenContinuity`，`LSMinimumSystemVersion` 26.5，**无 URL scheme、无公共 API**。因此该象限我们只能检测与调起，不能编程控制。
- 新建 `docs/CONTROL_ARCHITECTURE.md` 作为控制方向的唯一权威依据。
- 原计划文档加修订说明，并改写第一节路线、第四.4 节 iPhone 被控端、第五节 P0 实验、第十一节 iOS 发布、第十三节发布路径。原文其余部分（Android 方案、安全设计、协议设计、测试矩阵）仍然有效，未改动。
- **一次自我纠正**：初稿把 HID 写成"实测证伪"，但我实测到的恰恰相反——SDP 记录发布成功、两个 PSM 均注册成功；真正未验证的是 CoD、可发现状态和 iPhone 是否配对。已在计划、CONTROL_ARCHITECTURE、README、MACOS_HID_FEASIBILITY 四处改为"因失去用途而中止，非证伪"。
- 同时发现：先前转述的 Codex 结论（bluetoothd 占用 PSM 等）无法取回原文核实（作业状态已清除），因此**未采信、未写入任何文档**。
- 目录调整：移除 `extensions/ios-broadcast`（ReplayKit 出范围）；新增 `agents/ios-wda`（Windows→iPhone 象限）；改写 windows/macos/ios 三个 app README 以反映各自职责。
- Apple 诊断栈与 iOS app 明确标注为**非产品路径**，保留为连接诊断工具。
- 已更新 README、STATUS、ARCHITECTURE、DECISIONS、RISKS、PRODUCT_REQUIREMENTS、IOS_FEASIBILITY、MACOS_HID_FEASIBILITY、CONNECT_AND_TEST。
