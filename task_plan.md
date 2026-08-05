# QuadControl 实施计划

## Task Status

**Complete for the implemented M0 scope, with explicit toolchain blockers** — macOS 连接诊断闭环已完成并通过干净构建/self-test/独立终审；iOS 源码与真机步骤已完成，但完整 Xcode/签名/真机证据仍 Blocked；Rust/ADB 验证仍因工具链缺失 Blocked。没有宣称投屏或控制已实现。

## 目标

完成现有项目计划中“首个任务”的基础代码：建立 Monorepo 与 Rust workspace、四端最小工程说明、核心文档，以及可测试的 Android ADB 能力探测 CLI；不制作正式 UI，不宣称未经真机验证的 iOS/熄屏能力。

## 可验证完成条件

- 项目根目录存在用户指定的 `AGENTS.md` 编排规则。
- Rust workspace 能构建，包含 `core`、`protocol`、`transport`、`crypto`、`diagnostics`、`media`、`pairing`、`ffi` 以及 ADB probe。
- ADB probe 位于 `agents/android-shell/adb-probe`，能报告 adb 版本、已连接设备、USB/TCP/模拟器/未知传输类型和 mDNS 无线配对/连接服务；`pairing_service_advertised`、`connect_service_advertised`、`wireless_connected` 分开报告，`paired_with_this_host` 明确为当前只读探测不可可靠观测；adb 缺失或命令失败时返回清晰诊断。
- ADB probe 测试覆盖正常、离线、未授权、USB、TCP、模拟器、未知传输、两类 mDNS 服务、畸形/重复/空输出、adb 缺失、非零退出、stderr、超时、子命令不支持、结构化输出和退出码。
- Windows、macOS、Android、iOS 与 iOS Broadcast Extension 都有最小可编译工程的边界/命令说明；当前主机不可验证的平台不得标记已构建。
- 仓库至少包含 `apps/{windows,macos,android,ios}`、`extensions/ios-broadcast`、`agents/android-shell`、`crates/{core,protocol,transport,crypto,media,pairing,diagnostics,ffi}`、`services/{signaling,relay}`、`tests/{protocol,integration,security,device-lab}`、`LICENSES`、`docs`、`scripts`；不依赖 Git 跟踪空目录，使用说明文件表达占位边界。
- 核心文档逐项存在：`PRODUCT_REQUIREMENTS.md`、`PRODUCT_CONSTRAINTS.md`、`ARCHITECTURE.md`、`PROTOCOL.md`、`SECURITY.md`、`THREAT_MODEL.md`、`IOS_FEASIBILITY.md`、`ANDROID_COMPATIBILITY.md`、`DECISIONS.md`、`RISKS.md`、`STATUS.md`，并明确锁屏、熄屏、主动授权和实验能力边界。
- `cargo metadata --no-deps --format-version 1`、`cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`scripts/verify-all.sh` 在可用工具链范围内通过；脚本对缺失工具只可报告明确 `SKIP`，不能计为 `PASS`。

## 阶段

| 阶段 | 状态 | 可观察结果 / 验证方式 |
|---|---|---|
| 1. 锁定范围与架构 | complete | 独立评审计划；记录技术边界和目录映射 |
| 2. 建立仓库骨架与文档 | complete | 完整目录、workspace、核心文档、四端说明存在 |
| 3. 实现 ADB probe 与测试 | complete | CLI 结构化输出；24 项单元/集成测试已编写 |
| 4. 主协调器验收与修正 | blocked | 已读全部文件并修正；静态检查通过，Rust 工具链验证未执行 |
| 5. 代码 vs 方案独立评审 | complete | 唯一 major 已修复；复核仅保留 Rust 实际验证阻塞 |
| 6. Apple 闭环范围与工具盘点 | complete | 已读取插件 skill、确认工具状态并完成独立安全方案评审 |
| 7. macOS 连接诊断端 | complete | SwiftPM build 与 51 项 self-test 断言通过；非交互 token 输出拒绝通过 |
| 8. iOS SwiftUI 连接客户端 | complete (source) / blocked (runtime) | 源码、Info.plist、tests、XcodeGen scheme 与静态 parse/lint 通过；Xcode build/Simulator/签名/真机受环境阻塞 |
| 9. 连接与真机测试文档 | complete | 已给出 Mac+iPhone 真机步骤、预期日志、故障排查及 Android 当前只读 ADB 边界 |
| 10. 主验收与独立一致性复核 | complete | 干净 Swift build、51 项 self-test、静态检查和非交互拒绝通过；独立终审 PASS，无 blocking/major/minor |

## 范围约束

- 不开始正式 UI、媒体传输、输入注入、远程中继或真实设备控制。
- 不使用私有 API、越狱、Root、锁屏绕过、隐藏控制或默认无人值守。
- iOS ReplayKit、Screen Curtain 与 HID 只建立文档和实验边界，不标记可用。
- 不引入额外编排框架、额外 MCP 或 agent swarm。
- 不创建提交、tag 或合并分支；当前目录尚未初始化 Git，发布收尾不在本任务范围。
- ADB probe 是被动诊断工具：只允许参数化执行 `adb version`、`adb devices -l`、`adb mdns services`；禁止 `pair`、`connect`、`tcpip`、`shell`、`kill-server` 及任何设备修改命令。
- 每个 ADB 子进程必须有超时和 stdout/stderr 尺寸上限；文档明确探测可能启动本机 adb server，默认不持久化设备序列号或局域网地址。
- Apple 本轮仅发送连接诊断消息，不发送屏幕、输入、剪贴板、文件或凭据；临时 token 不写入日志或持久存储。
- 连接诊断必须是用户在手机端主动发起、Mac 端可见，并具备明确断开；不得监听公网地址或实现无人值守。
- 若只实现认证而未实现完整会话加密，消息内容必须保持非敏感并在文档/UI 中标为开发诊断协议，不得复用于正式控制通道。
- Apple 诊断 wire contract 必须先写入 `proto/quadcontrol/v1/session.proto`；实际 Swift adapter 使用 4-byte big-endian length framing、最大 4096 bytes，并为 HMAC 使用唯一固定 transcript 编码与 golden vectors，不直接签任意 JSON 字节。
- token 必须由 32 个随机字节组成、TTL 180 秒、只允许一个成功会话；握手包含双方 32-byte nonce、16-byte challenge/server/session ID、client proof、server finished 与 HKDF session key。token/challenge 在首次成功后原子消费。
- 心跳每 5 秒发送，15 秒无有效心跳断开；每帧带严格递增序号和 session MAC。未知版本、错误长度、乱序状态、重复/回退序号立即断开。
- 全局 pending 握手最多 8 个、有效会话最多 1 个；失败尝试需有限速。默认仅 loopback；LAN 必须显式 `--lan --interface wifi|wired`，并限制指定接口和私有/link-local peer。
- token 禁止进入命令行、URL、Bonjour、环境变量、UserDefaults、Keychain、文件、日志、遥测或崩溃上下文；Mac 只在交互式终端显示一次，iOS 使用安全输入并在后台/成功/失败/超时清空。
- iOS 诊断客户端只接受数字形式的 loopback、RFC1918、ULA 或 link-local 地址；拒绝域名和公网数字地址，避免把未加密的诊断消息发往远程目标。

## 决策

- 以现有 957 行项目计划为产品与架构依据。
- 首个可执行纵切是 Rust ADB 能力探测 CLI；四端本轮只提供工程边界和构建说明。
- 无法调用计划中写明但当前运行时未提供的 `gpt-5.6-luna`；实现委托使用可用的 Codex 会话，主协调器仍执行完整验收。
- `adb mdns services` 只能证明服务被广播，不能证明本机已与设备配对；CLI 不得据此推断配对成立。
- Apple M0 连接测试采用高熵一次性 token 的 challenge-response，只验证同一局域网内的用户授权连接；正式媒体/控制会话仍需独立的端到端加密协议。
- 当前只存在 Xcode Command Line Tools；iOS `.app`、Simulator 和真机构建必须保持未验证，直到完整 Xcode、签名与设备授权可用。
- 根级 Swift package 提供唯一的 `QuadControlDiagnosticProtocol` 与 `QuadControlMacListener`；iOS source/manifest 通过本地 package product 复用 frames/auth/limits，不复制协议。

## Errors Encountered

| Error | Attempt | Resolution |
|---|---:|---|
| `git status` 报当前目录不是 Git 仓库 | 1 | 记录为现状；本任务不擅自 `git init` |
| 初版将 mDNS 服务描述为“无线配对状态” | 1 | 独立评审指出不可可靠推断；已拆分可观察事实并把主机配对状态标为 unknown |
| Rust Docker 镜像首次拉取长时间无进展 | 2 | 停止重复拉取；尝试已有 Ubuntu 容器的临时工具链 |
| Ubuntu 容器 `apt-get update` 长时间停滞 | 1 | 停止临时容器；改为主机临时 rustup 下载 |
| 临时 rustup 下载速率约 25KB/s，完整工具链预计耗时过长 | 1 | 停止下载并清理临时文件；保留静态审查与脚本验证，Rust 编译/测试明确标为未证明 |
| 首次必需文件检查在 zsh 中未拆分空格字符串 | 1 | 改用显式文件参数列表 |
| 静态检查循环误用 zsh 特殊变量 `path`，覆盖命令搜索路径 | 1 | 改用普通变量 `file` 后重跑 |
| 尾随空白检查命中用户原始计划中的既有 Markdown 换行空格 | 1 | 保留原始计划不改；只对本任务新增文件执行检查并通过 |
| XcodeBuildMCP `list_sims` 找不到 `simctl` | 1 | 确认当前仅 Command Line Tools；不自行启动 Simulator，iOS 运行验证标为 Blocked |
| Apple 方案评审发现 wire contract 与认证状态机不完整 | 1 | 先补 `.proto`、固定 transcript、双向 proof、session MAC、TTL/限速/接口/日志合同后再实现 |
| 默认 `swift package describe/build/test` 被 SwiftPM `sandbox-exec` 拒绝 | 1 | 当前受管环境使用 `--disable-sandbox` 验证；包无第三方依赖或插件，普通终端仍优先默认命令 |
| Command Line Tools 同时缺少 `XCTest` 与 `Testing` 模块 | 1 | 不接受 `swift test` 的零测试退出 0；增加可执行 self-test 覆盖核心与 loopback，完整 Xcode 后再运行 XCTest/iOS tests |
| 首次重构后 Swift build 报顶层 private 类型可见性及 throwing autoclosure 错误 | 1 | 将顶层实例收紧为 private，并先求值可抛表达式再断言；重跑 build 通过 |
| 更新计划文件时 patch 上下文多写了“断开”一词而未命中 | 1 | 读取精确行后缩小 patch 上下文重试；没有产品代码受影响 |
