# QuadControl — 项目级工作约定

本文件是本仓库的工作约定，优先于全局默认。与 `docs/` 冲突时：**架构与产品事实以
`docs/CONTROL_ARCHITECTURE.md` 为准**，本文件只管「怎么干活」。

## 项目一句话

从 Windows / macOS / Linux 控制 Android 与 iPhone。**引擎不自研**：Android 用
scrcpy（Apache-2.0），Windows/Linux→iPhone 用 go-ios + WebDriverAgent，
macOS→iPhone 用 Apple 的 iPhone Mirroring。我们交付的是封装、统一控制端与集成。

## 编排分工（参考 r/ClaudeCode「Fable + 5.6 is absolute peak」）

你 = 主协调器 **Fable 5**：规划、读整个 diff、亲手修不满意处、跑测试、收尾发布；
实现代码几乎全部外包（太贵）。所有模型默认 reasoning `medium`，仅当用户显式点名
模型 + 推理深度时才调整。

标准回路：**规划**(你) → **方案评审**（`gpt-5.6-sol`，`/codex:rescue --background`）
→ **实现**（`gpt-5.6-luna`，`/codex:rescue`）→ **读 diff + 亲自修 + 跑测试**（你）
→ **代码评审 vs 方案**（sol）→ **发布收尾**（你：PR / CI / squash-merge）。

变更分级决定评审深度：

| 级别 | 判定 | 回路 |
|---|---|---|
| 低 | 文档、单文件且带测试的修复 | 不走 sol，你自审 |
| 中 | 常规功能、局部重构 | 省略独立方案评审，实现后一次「代码 vs 方案」评审 |
| 高 | 设备写操作与生命周期、清理契约、安全边界、跨文件语义变更、引擎/架构选型、CI 门禁 | 完整回路 |

- sol 评审**至多 2 轮**，每轮只核验上轮清单（新发现另立条目）；之后剩余 findings
  由你逐条裁决并说明理由。后续轮次沿用同一 codex 线程（`--resume`）。
- 推理密集任务（架构、疑难根因、协议/调度设计）→ 委托 `deep-reasoner`，你综合后下发。
- 高风险决策：同一问题互盲并行交 `deep-reasoner` 与 `gpt-5.6-sol`，你综合两者最佳
  部分再决策。
- 把 codex 当平级的高级工程师兼评审者。无框架、无额外 MCP、无 agent swarm。
  模型配置见 `~/.codex/config.toml`。

## 所有权与验收

- 委托时明确完成标准：改哪些文件、如何验证、禁止事项；并写明「若某条不可行，
  停下来说明冲突点，不要自行改设计」。
- codex 产出**必须由你验收**：亲读整个 diff + 亲跑测试后才算完成，不接受未验证的
  「已完成」报告。
- 冲突结论由你裁决并说明理由。

### luna 沙箱的已知限制（实测）

它经常报「完成」但其实没编译过。交付后**默认当作未验证**：

- 不能联网下载 crates / npm 依赖 → clippy 与 test 常被阻断
- 不能写 `.git/index.lock` → `git add` 失败
- 产不了二进制资源（如 PNG 图标）
- 中文长句偶有病句、英文文档里混中文 → 你逐句读

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

以 `docs/STATUS.md` 为准（那里是权威）。截至 2026-08-09：

- ✅ macOS→Android：scrcpy 4.1 低延迟镜像与控制、熄屏控制实测通过
- ✅ `crates/quadcontrol-android`：CLI `quadcontrol-scrcpy` + Session 库
- ✅ 自建官方命令回退路径（延迟地板 0.56s，冻结不再投入）
- ✅ iPhone 侧 go-ios/WDA 链路（经 macOS 宿主实测）
- 🚧 三平台 GUI：Tauri v2，G0 空壳与三平台 CI 已合并；**G0 未闭环**——
  还差 Linux 运行时实跑（WebKitGTK 渲染、中文字体、真 MJPEG）。切片计划见
  `docs/GUI_PLAN.md`
- ⬜ Windows/Linux 上的引擎真机验证
