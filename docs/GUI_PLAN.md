# 三平台统一控制端 GUI 方案（终稿；sol 两轮评审通过，栈 = Tauri v2 用户拍板）

## 地基事实
1. Android 象限的视频窗口归 scrcpy（SDL 自渲染）；GUI 是指挥台：发现→配对→启动→会话管理，不渲染 Android 视频。
2. iPhone 象限（Win/Linux）需 GUI 内渲染 WDA 的 MJPEG 流并转发点击/文字；每会话独立本地端口（go-ios forward），CSP `img-src` 仅 localhost。**不标 trivial**——G0 用真流验证。
3. macOS→iPhone：检测 + `open -b com.apple.ScreenContinuity` 引导卡。
4. 产品路径上可复用的跨平台控制资产主要是 Rust：`quadcontrol-android`（Session 库，scrcpy 生命周期已评审）、`adb_probe`（设备解析，只读三命令约束不动）；go-ios 为外部二进制。仓库另有 Swift 诊断与回退实现，不进 GUI 路径。
5. 双语硬约束（zh-Hans/en）；所有会话可见、可断开（安全边界）；设备刷新率选项在 GUI 中根本不存在（红线）。

## 栈：Tauri v2（后备 Slint）
- Rust 后端直接 crate 依赖既有库，零桥接；Web 前端只做指挥台 UI（保持薄）。
- 所有阻塞调用（Session、设备轮询、go-ios）走 async command + `spawn_blocking`；同步 command 禁用于任何阻塞工作（Tauri 同步命令在主线程执行）。
- 若 G0 Linux spike 失败 → 显式止损切 **Slint**（Rust 原生、无 WebView 依赖）。
- 候选对比结论存档：iced（retained）/egui（immediate）UI 费工生态小；Flutter 需桥接层（通常 flutter_rust_bridge）扩大维护面；Electron 桥接最绕且重；原生三套违背统一目标。

## 会话所有权模型（G1.5 库 PR，G2 前置）
- 监督线程**独占** `Session`，是唯一调用其方法的执行者；经命令通道收 stop。
- 库新增 `try_wait`（非消费轮询）与 `SessionHandle`（id + 状态快照 + stop 发送端）；Tauri command 只碰 Handle。
- 退出序列：拦截 exit → 广播 stop → **有界等待，期限由库统一暴露且大于终止梯子总时长**（Unix 梯 3s+2s → 等待 ≥7s），超时强制回收（kill + reap）并记录 → 退出。
- Windows：**G2 引入 Job Object（kill-on-close）保证进程树回收**——GUI「一键断开」是安全边界宣称，硬杀降级契约不足以支撑。CLI 有 console 路径维持现状。

## 切片（每片完整回路：sol 方案确认→luna→Fable 复核冒烟→sol 代码审→CI→合并）
- **G0 风险切片**：Tauri 空壳三平台 CI 构建 + **Linux 基线 Ubuntu 22.04 LTS 实跑**。运行时验证在本机 UTM 虚拟机（**arm64**，22.04 无官方 arm64 桌面 ISO → server ISO + `ubuntu-desktop` 包；用户 2026-08-08 拍板 VM 路线）；x86_64 由 CI 构建覆盖（Wayland 与 X11、中文字体、真 WDA MJPEG + CSP 均在 VM 实跑）。通过阈值：窗口正常渲染中文、MJPEG ≥10fps 持续 5 分钟无泄漏。**触发 Slint 的失败**：WebKitGTK 无法在基线发行版渲染窗口/崩溃、MJPEG 无法达标；**不触发**：打包脚本、字体配置类可修复问题。
- **G1 设备列表**：adb（probe 解析）+ `ios list` 双列，状态徽章，双语。
- **G1.5 会话监督库 PR**：上述所有权模型 + 测试。
- **G2 Android 一键会话**：起停按钮、状态卡（运行/退出码/stderr 尾）、熄屏与码率选项；**Windows Job Object + Windows 真机无孤儿与 forward 清理进合并门禁**；macOS 真机验收。
- **G3 配对向导**：`adb pair/connect` 为**新的有状态写操作**，独立设计参数验证、超时、输出上限、安全边界（不冒充 probe 延伸）。
- **G4 iPhone 面板**：WDA MJPEG + 点击/文字转发（Win/Linux）；macOS 引导卡。
- **G5 打包发布**：安装包、版本、更新说明。

## 文档
- G1 落地时更新 STATUS/架构文档；每片合并时同步。
