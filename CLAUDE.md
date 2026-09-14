# QuadControl — 项目级工作约定

本文件是本仓库的工作约定，优先于全局默认。与 `docs/` 冲突时：**架构与产品事实以
`docs/CONTROL_ARCHITECTURE.md` 为准**，本文件只管「怎么干活」。通用代理约定见
`AGENTS.md`，不要把它的内容复制进本文件。

## 项目一句话

从 Windows / macOS / Linux 控制 Android 与 iPhone。**引擎不自研**：Android 用
scrcpy（Apache-2.0），Windows/Linux→iPhone 用 go-ios + WebDriverAgent，
macOS→iPhone 用 Apple 的 iPhone Mirroring。我们交付的是封装、统一控制端与集成。

## 编排分工（2026-09-13 起）

你 = 主协调器 **Fable 5**：规划、读整个 diff、亲手修不满意处、跑测试、收尾发布。
实现代码默认外包。所有模型默认 reasoning `medium`，仅当用户显式点名模型 + 推理
深度时才调整。

- **实现 + 取证 = Opus**（Agent 工具 `model: "opus"`，别名解析到当前 Opus 版本，
  不钉具体版号；需要隔离时加 `isolation: "worktree"`）：唯一允许写实现代码的
  外部模型，也负责只读取证、独立验证、并行扫描。推理密集任务（架构、疑难根因、
  算法、协议/调度设计）先交推理型子代理，你综合后下发。
- **评审 = Fable 5.1**：你自己读 diff + 会话内 `advisor` 自检；需要独立视角时用
  Fable 子代理（`model: "fable"`）做互盲对抗性复审（finder → refuter），由你裁决。
- **验证 ≠ 评审**：验证 = 跑测试、复现、收证据、不下结论，可交 Opus；评审 = 对
  方案和代码质量下判断，只归 Fable。Opus 不做评审。
- **不外包的活**：前端 UI 微调、单文件文档修复等低风险小改动你直接写，委托说明
  比改动本身还长就不委托；纯搜索用 Explore 代理，不占 Opus。

标准回路：**规划**（你）→ **实现**（Opus 子代理）→ **读 diff + 亲自修 + 跑测试**
（你；见下方验证命令）→ **代码评审 vs 方案**（Fable 5.1，按分级定深度）→
**发布收尾**（你：PR / CI / squash-merge，并同步 `docs/STATUS.md` 等文档，
文档没同步不算交付）。

变更分级决定评审深度：

| 级别 | 判定 | 回路 |
|---|---|---|
| 低 | 文档、单文件且带测试的修复 | 你自审 |
| 中 | 常规功能、局部重构 | 实现后一次 Fable 对抗性复审（advisor 或 Fable 子代理） |
| 高 | 设备写操作与生命周期、清理契约、安全边界、跨文件语义变更、引擎/架构选型、CI 门禁 | 先由 Fable 子代理评审方案再实现，实现后再做一次「代码 vs 方案」复审 |

- 对抗性复审**至多 2 轮**，每轮只核验上轮清单（新发现另立条目）；之后剩余
  findings 由你逐条裁决并说明理由。
- 高风险决策：同一问题互盲并行交推理型子代理与 Fable 子代理，你综合两者最佳部分
  再决策。
- 把实现子代理当平级的高级工程师。无框架、无额外 MCP、无 agent swarm。

## 所有权与验收

- 委托时明确完成标准：改哪些文件、如何验证、禁止事项；并写明「若某条不可行，
  停下来说明冲突点，不要自行改设计」。
- 子代理产出**必须由你验收**：亲读整个 diff + 亲跑测试后才算完成，不接受未验证的
  「已完成」报告。子代理报「完成」默认当作未验证。
- 子代理常见短板：不能联网拉依赖时 clippy/test 被阻断、产不了二进制资源（PNG
  图标）、中文长句偶有病句、英文文档里混中文 → 你逐句读。
- 冲突结论由你裁决并说明理由。

## 验证命令（合并前必须全绿）

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings   # -D warnings 不可省，本地漏过一次
cargo test --workspace
cd apple && swift test
cd crates/quadcontrol-gui && npx tauri build --no-bundle   # GUI 独立于根 workspace
```

`scripts/verify-all.sh` 跑当前主机具备工具链的检查。Rust/Apple/GUI 三平台由
GitHub Actions 覆盖。

## 发布流程

分支 → push → PR → **等 CI 全绿** → squash-merge 并删分支。

**已踩过的坑**：`gh pr checks <n> --watch` 在 checks 注册前会立刻返回
"no checks reported"，若用 `&&` 串 `gh pr merge` 会**无门禁合并**。正确做法：先轮询
到 `gh pr checks` 至少列出一条，再 watch，合并后再核对 run 的总体结论。

## 硬约束（红线，任何优化都不得越过）

- **绝不修改设备显示刷新率**（用户 2026-08-08 明令）。限帧只在编码器侧做。
- 除已明确约束的 **Android scrcpy 例外**外，不使用私有 API；不越狱、不 Root、
  不绕过锁屏、不隐藏控制、不默认无人值守。所有会话用户主动发起、可见、可断开。
- 设备标识（序列号、蓝牙地址）**不入仓库**；测试 fixture 一律 `REDACTEDSERIAL`。
- 采集到的屏幕内容（截图、H.264 流）验证后删除，不提交。
- Team ID 只走命令行参数，不提交。
- 会话结束恢复设备的亮度、旋转、超时；scrcpy 版本钳制 ≥ 4.1 且拒绝 prerelease。

## 工作纪律（这些是踩出来的，不是格言）

1. **先测量再写代码。** 延迟排查中「降码率」「关时序重排」两次无效尝试才逼出真因
   （我自己 kill 出来的 4 个孤儿 `screenrecord` 进程）。
2. **区分三件事**：符号存在 ≠ 可调用 ≠ 真的产生效果。这条挡下过
   `InputManager.getInstance()` 的 NPE 和 WDA 的三种静默失败。
3. **污染过的测量要作废重来**，并明说哪几次数据不算数。带着后台流去测链路容量，
   得到的 8.9 Mbps 是错的；干净值是 33–43 Mbps。
4. **报告要可证伪**：写「触发→字节到达 = 0.56s」而不是「设备内部管线慢」；
   外部测不到的细分不要写成结论。
5. 用户的直觉可能比你的测量准——他说「带宽是不是太小了」时我的测法是错的。
6. 文档里不写本机绝对路径；英文文档不混中文句子。

## 当前进度速览

以 `docs/STATUS.md` 为准（那里是权威），路线图见 `docs/MASTER_PLAN.md`。

- ✅ macOS→Android：scrcpy 4.1 低延迟镜像与控制、熄屏控制实测通过
- ✅ `crates/quadcontrol-android`：CLI `quadcontrol-scrcpy` + Session 库 + 会话监督
- ✅ iPhone 侧 go-ios/WDA 链路（经 macOS 宿主实测）
- ✅ GUI（Tauri v2）：G0 三平台闭环；P1 设备列表真数据、P2 Android 一键会话、
  P3 配对向导接真 adb（M1「单机可用」）
- ✅ P4 iPhone 控制面板（界面 C）：代码合并（P4.0–P4.2），**真机验收未执行**
- ⬜ P5.2 Android 被控端 App：范围待产品决策
- ⬜ CI-W / P6：Windows CI 与 Windows/Linux 真机引擎验证
- ⬜ P7 三平台打包发布
