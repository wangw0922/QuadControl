# 编排分工（Codex 版）

本文件面向以 Codex 为主协调器的会话。以 Claude Code 为主协调器时用
[`CLAUDE.md`](CLAUDE.md)，两者的项目事实、红线与验收标准必须保持一致。

与 `docs/` 冲突时：**架构与产品事实以 `docs/CONTROL_ARCHITECTURE.md` 为准**，
进度以 `docs/STATUS.md` 为准；本文件只管「怎么干活」。

## 项目一句话

QuadControl：从 Windows / macOS / Linux 控制 Android 与 iPhone。**引擎不自研**
——Android 用 scrcpy（Apache-2.0），Windows/Linux→iPhone 用 go-ios +
WebDriverAgent，macOS→iPhone 用 Apple 的 iPhone Mirroring。我们交付封装、
统一控制端与集成。做方案前先读架构文档，不要重新发明已被否决的路线。

## 角色与回路

你 = 主协调器 **GPT-5.6 Sol**，推理深度固定 `x high`：负责规划、读取完整 diff、
亲手修正不满意之处、运行测试以及发布收尾。实现代码原则上委托出去，以控制主
协调器成本。除主协调器外，所有委托模型默认 reasoning `medium`；仅当用户显式
指定模型和推理深度时才调整。

标准回路：

**规划**（你，`gpt-5.6-sol` / `x high`）
→ **方案评审**（独立 `gpt-5.6-sol`，`/codex:rescue --background`）
→ **实现**（`gpt-5.6-luna`，`/codex:rescue`）
→ **读取完整 diff + 亲自修正 + 运行测试**（你）
→ **代码与方案一致性评审**（独立 `gpt-5.6-sol`）
→ **发布收尾**（你：PR、等 CI、squash-merge、必要的回滚说明）。

评审深度与变更分级挂钩：

| 级别 | 判定 | 回路 |
|---|---|---|
| 低风险 | 文档修改、带测试的单文件修复 | 无需独立 Sol 评审，你自审、验证并收尾 |
| 中风险 | 常规功能、局部重构 | 省略独立方案评审；实现后执行一次「代码 vs 方案」评审 |
| 高风险 | 设备写操作与生命周期、清理契约、安全边界、跨文件语义变更、引擎/架构选型、CI 门禁 | 必须完整回路 |

- 独立 Sol 评审**至多 2 轮**，每轮只核验上轮清单（新发现另立条目）；之后剩余
  findings 由你逐条裁决并说明理由。后续轮次沿用同一 codex 线程（`--resume`）。
- 推理密集型任务（架构设计、疑难根因、复杂算法、采集调度或权限闭环设计）委托
  `deep-reasoner`；你综合其结论、处理冲突并形成可执行方案，再下发实现。
- 高风险决策：同一问题互盲并行交 `deep-reasoner` 与独立 `gpt-5.6-sol`，你综合
  两者最佳部分再决策。
- 将其他 Codex 会话视为平级的高级工程师和独立评审者，而不是无条件可信的执行器。
- 不引入额外编排框架、额外 MCP 或 agent swarm。模型与 profile 配置统一以
  `~/.codex/config.toml` 为准。

## 所有权与验收

每次委托必须明确完成标准，包括：

- 需要修改的文件或允许修改的范围；
- 预期行为和验收条件；
- 必须运行的验证命令；
- 禁止修改的内容及其他约束；
- 「若方案某条在实现中发现不可行，停下来说明冲突点，不要自行改设计」。

Codex 的产出必须由你亲自验收。只有在你读取完整 diff、检查关键实现并亲自运行
测试后，任务才可视为完成。不接受未经验证的「已完成」「测试应当通过」或仅基于
局部 diff 得出的完成报告。

委托模型之间出现冲突结论时由你裁决，依据代码、测试结果、需求约束和风险等级，
并在最终说明中给出理由。最终发布责任始终由你承担。

### luna 沙箱的已知限制（本项目实测）

它常报「完成」但其实没编译过，交付后**默认当作未验证**：

- 不能联网下载 crates / npm 依赖 → clippy 与 test 常被阻断；
- 不能写 `.git/index.lock` → `git add` 失败；
- 产不了二进制资源（如 PNG 图标）；
- 中文长句偶有病句、英文文档里混入中文 → 逐句读过再收。

## 验证命令（合并前必须全绿）

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings   # -D warnings 不可省，曾漏过导致 CI 挂
cargo test --workspace
cd apple && swift test
cd crates/quadcontrol-gui && npx tauri build --no-bundle   # GUI 独立于根 workspace
```

`scripts/verify-all.sh` 跑当前主机具备工具链的检查。Rust / Apple / GUI 三平台由
GitHub Actions 覆盖（Rust 与 GUI 均为 ubuntu + windows + macos matrix）。

## 发布流程

分支 → push → PR → **等 CI 全绿** → squash-merge 并删分支。

**已踩过的坑**：`gh pr checks <n> --watch` 在 checks 注册前会立刻返回
"no checks reported"，若用 `&&` 串 `gh pr merge` 会**无门禁合并**。正确做法：先
轮询到 `gh pr checks` 至少列出一条，再 watch，合并后再核对 run 的总体结论。

## 硬约束（红线，任何优化都不得越过）

- **绝不修改设备显示刷新率**（用户 2026-08-08 明令）。限帧只在编码器侧做。
- 除已明确约束的 **Android scrcpy 例外**外，不使用私有 API；不越狱、不 Root、
  不绕过锁屏、不隐藏控制、不默认无人值守。所有会话由用户主动发起、可见、可断开。
- 设备标识（序列号、蓝牙地址）**不入仓库**；测试 fixture 一律 `REDACTEDSERIAL`。
- 采集到的屏幕内容（截图、H.264 流）验证后删除，不提交。
- Team ID 只走命令行参数，不提交。
- 会话结束恢复设备的亮度、旋转、超时；scrcpy 版本钳制 ≥ 4.1 且拒绝 prerelease。

## 工作纪律（踩出来的，不是格言）

1. **先测量再写代码。** 延迟排查中「降码率」「关时序重排」两次无效尝试才逼出
   真因（自己 kill 出来的 4 个孤儿 `screenrecord` 进程）。
2. **区分三件事**：符号存在 ≠ 可调用 ≠ 真的产生效果。这条挡下过
   `InputManager.getInstance()` 的 NPE 和 WDA 的三种静默失败。
3. **污染过的测量要作废重来**，并明说哪几次数据不算数。带着后台流测链路容量
   得到的 8.9 Mbps 是错的；干净值是 33–43 Mbps。
4. **报告要可证伪**：写「触发→字节到达 = 0.56s」而不是「设备内部管线慢」；
   外部测不到的细分不要写成结论。
5. 用户的直觉可能比你的测量准——他说「带宽是不是太小了」时，是测法错了。
6. 文档里不写本机绝对路径；英文文档不混中文句子。

## 当前进度速览

以 `docs/STATUS.md` 为准。截至 2026-08-09：

- ✅ macOS→Android：scrcpy 4.1 低延迟镜像与控制、熄屏控制实测通过
- ✅ `crates/quadcontrol-android`：CLI `quadcontrol-scrcpy` + Session 库
- ✅ 自建官方命令回退路径（延迟地板 0.56s，冻结不再投入）
- ✅ iPhone 侧 go-ios / WDA 链路（经 macOS 宿主实测）
- 🚧 三平台 GUI：Tauri v2，G0 空壳与三平台 CI 已合并；**G0 未闭环**——还差
  Linux 运行时实跑（WebKitGTK 渲染、中文字体、真 MJPEG）。切片计划见
  `docs/GUI_PLAN.md`
- ⬜ Windows / Linux 上的引擎真机验证
