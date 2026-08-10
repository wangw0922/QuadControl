# Handoff: QuadControl 四端 UI（第 1 版草稿）

## Overview
QuadControl 的全套界面设计：桌面控制端（Mac / Windows / Linux，Tauri v2 指挥台）与被控端 App（iOS SwiftUI、Android）。共 5 个界面：A 桌面主界面（设备列表 + 会话）、B 配对向导、C iPhone 控制面板（Win/Linux）、D iOS 被控端、E Android 被控端。全部文案为简体中文，面向普通用户。

## About the Design Files
本包内的文件是 **HTML 设计参考稿**（原型），展示预期的外观与行为，**不是可直接搬运的生产代码**。任务是：在目标代码库既有环境中**重新实现**这些设计——桌面端用仓库已定稿的 Tauri v2 Web 前端，iOS 端用现有 SwiftUI 工程（`apps/ios/`），Android 端按仓库规划实现。遵循代码库既有模式与约束（见下"硬约束"）。

打开方式：`QuadControl UI 草稿.dc.html` 直接用浏览器打开即可预览（同目录需保留 styles.css、support.js、三个 .jsx 设备外框文件）。macOS 窗口/iPhone/Android 的**外框仅为展示用途**，不需要实现；实现的是框内内容。

## Fidelity
**高保真（hifi）**。颜色、字体、圆角、间距、文案均为最终意图，应按下方设计令牌精确还原。字体例外：Caprasimo/Figtree 对中文无字形，中文环境落到系统字体即可，字号字重按规范。

## 硬约束（来自仓库文档，不可违反）
- 双语：所有用户可见字符串走本地化资源（zh-Hans + en），不得硬编码在视图里（机器可读输出保持英文）。
- 所有会话可见、可断开；不得隐藏控制；不绕过锁屏/密码/Face ID。
- GUI 中**不存在**设备刷新率选项（红线）。
- Android 视频窗口归 scrcpy 自渲染，GUI 不渲染 Android 视频（对应界面 A 的"镜像窗口"占位说明）。
- iPhone 象限（Win/Linux）在 GUI 内渲染 WDA MJPEG 流（界面 C 左侧画面区）。
- macOS→iPhone 不自研：只出引导卡（界面 C 底部的 sage 提示卡），动作为打开系统 iPhone 镜像。

## Screens / Views

### A. 桌面主界面 — 设备与会话（1100×680）
- 布局：左侧栏 264px（背景 `--color-surface` #ebddc5，padding 20px 14px）+ 主区（padding 28px 32px，列向 gap 20px）。
- 侧栏：小节标题「我的设备」（11px 大写风格，neutral-600）；设备行为全圆角胶囊（padding 12px 14px，gap 12px）：手机图标（Lucide smartphone，20px，stroke 2.75）+ 名称 14px/600 + 副文案 11px + 右侧 9px 状态圆点。选中行：背景 `--color-accent` #c67139、文字 #f5ead8。未选中 hover：`--color-neutral-200`。状态圆点：正在控制 accent-2-300，可连接 accent-2-500，需配对 neutral-400。底部「+ 添加设备」次级按钮（btn-secondary，全宽）。
- 主区标题行：设备名 h3（25px 标题字体）+ 标签「会话进行中」(tag-accent-2) +「Wi-Fi · 延迟 38 ms」(tag-neutral)；右侧按钮「显示镜像窗口」(secondary) 与「断开连接」（primary，背景加深为 `--color-accent-700`，white-space: nowrap）。
- 主区内容两列（gap 20px）：
  - 左（flex 1.2）镜像状态卡：45° 斜纹占位（surface/neutral-200 相间 14px），中央 120×240 圆角 24px 白卡内 smartphone 图标，monospace 12px 说明「手机画面在独立的镜像窗口中显示（scrcpy 实时画面 · 可直接点按操作）」。
  - 右（flex 1）控制卡（card elev-sm，行间 1px divider 分隔）三行，每行 = 图标(18px accent) + 标题 14px/600 + 副文案 12px muted + 右侧控件：
    1. 声音输出 — seg 分段控件「这台电脑 | 手机」，默认"这台电脑"。
    2. 手机熄屏 — seg「开 | 关」，默认关；副文案「熄灭手机屏幕，画面仍在电脑上显示」。
    3. 截取手机屏幕 — 「截屏」secondary 按钮；副文案「保存到这台电脑的"图片"文件夹」。
  - 右下安全提示卡：背景 `--color-accent-2-100`，文字 accent-2-800；标题「连接由你发起，随时可断开」+ 说明「控制期间手机上会一直显示提示；本软件不会绕过锁屏或密码。」

### B. 配对向导（模态对话框，760 宽窗口内）
- 背景：主界面上 45% 深色遮罩；对话框 `.dialog`（宽 ≤600px，圆角 --radius-lg×1.15，shadow-lg，背景 surface）。
- 步骤条：3 步「选择类型（✓，accent 圆内白勾）→ 扫码配对（当前，accent 实心圆 "2"）→ 开始控制（描边圆 "3"，muted）」，之间 1px 分隔线。
- 标题 24px：「用手机扫一下，就连好了」。
- 内容两列：左 216×216 二维码区（圆角 --radius-lg，背景 neutral-100，下注 monospace 11px「配对二维码 · 30 秒自动刷新」，实现时放真实 QR，内容 = host+port+token 配对负载）；右为有序列表 3 条（14px，gap 12px）：打开 QuadControl → 点「扫描配对码」→ 对准二维码自动填好。
- 注脚 12px muted：「二维码只在你的家庭/办公网络内有效，不会经过互联网。」
- 底部：左链接「无法扫码？手动输入配对码」；右「上一步」(secondary) +「等待手机扫码…」（primary，disabled 45% 透明度，扫码成功后变为进入第 3 步）。

### C. iPhone 控制面板（Windows/Linux，880×640）
- 布局：左固定列（画面）+ 右控制列（gap 24px，padding 24px 28px）。
- 左：246×500 圆角 28px 画面区 = WDA MJPEG 流（原型为斜纹占位）；点击画面即转发点按到手机。下方标签「已连接 · 15 帧/秒」(tag-accent-2)。
- 右：标题行 iPhone SE (h4) + 右侧「断开连接」primary（accent-700）。
  - 文字转发卡：label「发送文字到 iPhone」+ 输入框（pill 圆角，placeholder「在这里打字，直接输入到手机…」）+「发送」primary（flex-shrink 0）。
  - 操作按钮卡（横排 wrap，gap 10px）：亮屏（sun 图标）、截取屏幕（camera）、回到主屏幕（house）——均 secondary。
  - 说明卡（neutral-100）：「声音保留在手机上 / iPhone 的声音暂时无法转发到电脑，这是系统限制。」
  - 底部 macOS 引导卡（accent-2-100）：monitor 图标 + 「在 Mac 上？系统自带的「iPhone 镜像」体验更完整，QuadControl 会直接帮你打开它。」+ ghost 按钮「了解详情」（实现：`open -b com.apple.ScreenContinuity`）。

### D. iOS 被控端 App（402×874，SwiftUI）
- 顶部导航题「QuadControl」。内容列 padding 12px 20px 40px，gap 16px。
- 英雄区：96px 圆形 accent-100 底 + scan 图标（44px accent）；标题 h3「连接到你的电脑」；说明 14px muted 居中（max-width 280px）：「在电脑上打开 QuadControl，点「添加设备」，然后用下面的按钮扫电脑屏幕上的二维码。」
- 主按钮：全宽 pill primary，高 ≥50px、16px 字：「扫描配对码」（scan 图标）。次按钮 ghost：「手动输入地址和密钥」（展开 host/port/token 表单，复用现有 ConnectionModel 校验逻辑）。
- 状态卡：10px 圆点（未连接 neutral-400 / 已连接 accent-2-500）+「未连接」14px/600 + 右侧 tag「状态」。
- 底部安全注脚 12px muted 居中：「连接只能由你发起，连接期间屏幕上会一直显示提示，随时可以断开。不会绕过锁屏、密码或面容 ID。」
- 状态机沿用现有 `ConnectionStatus`（连接中/验证中/已连接/失败等本地化字符串已在 zh-Hans.lproj）。

### E. Android 被控端 App（412×892）
- 顶栏题「QuadControl」。内容列 padding 16px 20px 32px，gap 16px。
- 步骤卡（card elev-sm，kicker「三步连接电脑」accent 色小标题）：三行步骤，26px 圆形序号：
  1. ✓（accent-2-200 底/accent-2-800 勾）连接同一个 Wi-Fi — 副文案「已连接：Home-5G（和电脑相同）」；
  2. 当前（accent 实心）扫描电脑上的二维码 — 「在电脑的 QuadControl 里点「添加设备」」；
  3. 未来（描边、整体 50% 透明）在电脑上开始控制 — 「之后每次连接都不用再配对」。
- 主按钮：全宽 pill primary「扫码连接电脑」（scan 图标）。
- 状态卡：同 D。
- 可见性卡（accent-2-100）：「被控制时你一定看得见 / 电脑控制这台手机期间，通知栏会一直显示「正在被电脑控制」，点一下即可断开。」（实现为常驻前台通知）。
- 底部注脚：「支持熄屏控制：屏幕熄灭时，画面仍显示在电脑上。」

## Interactions & Behavior
- 侧栏设备行：hover neutral-200 底；点击切换主区会话详情；选中态 accent 实心。
- 「断开连接」立即终止会话（桌面端走会话监督线程的 stop 通道；有界等待后强制回收）。
- 配对向导：二维码 30 秒轮换；手机扫码成功 → 自动进入第 3 步并开始控制；「手动输入配对码」切换到表单视图。
- seg 分段控件：选中项 accent 实心底、白字；未选中 hover 7% 墨色 tint。
- 所有交互元素：hover 用 accent ramp 加深一档、按下再深一档；键盘焦点 `outline: 2px solid var(--color-accent); offset 2px`。
- 禁用态：45% 不透明度。
- 亮屏/截屏/回到主屏幕（iPhone）经 WDA；截屏（Android）保存到本机图片文件夹并弹系统通知。

## State Management
- 桌面端：设备列表（id、名称、平台、状态：正在控制/可连接/需要配对）；每设备一个会话句柄（运行状态、连接方式、延迟）；会话选项（声音输出：电脑|手机；熄屏：开|关）。
- 配对向导：step (1|2|3)、当前配对负载（host/port/token）、倒计时。
- 手机端：ConnectionStatus 状态机（iOS 已有实现，Android 对齐）。

## Design Tokens（styles.css 为准，全部用 CSS 变量，勿硬编码）
- 底色 `--color-bg` #f5ead8；表面 `--color-surface` #ebddc5；文字 `--color-text` #201e1d。
- 主强调 `--color-accent` #c67139（hover 600 #b2622d，按下 700 #8c491a）；第二强调 `--color-accent-2` #7a8a5e（浅底 100 #f0fae1，深字 800 #3d472b）。
- 中性 ramp neutral-100 #f9f4ed … 900 #2e2b25。
- 字体：标题 Caprasimo（中文回退系统字体），正文 Figtree；正文 15px/1.55。h2 32 / h3 25 / h4 20 / h5 16。
- 间距：4.4 / 8.8 / 13.2 / 17.6 / 26.4 / 35.2px（--space-1…8）。
- 圆角：sm 8 / md 16 / lg 28；按钮、输入框、seg 一律 999px 胶囊；卡片与对话框 ≈32px。
- 阴影：--shadow-sm/md/lg（墨色 tint，见 styles.css）。
- 图标：Lucide，stroke-width 2.75。

## Assets
- 图标全部来自 Lucide（smartphone, monitor, camera, volume-2, moon, sun, scan, qr-code, check, plus, power, house），无位图资源。
- 二维码为占位，实现时由配对负载实时生成。

## Files
- `QuadControl UI 草稿.dc.html` — 5 个界面的完整原型（浏览器直接打开）。
- `styles.css` — Organic 设计系统令牌 + 组件类（唯一样式来源）。
- `support.js` / `macos-window.jsx` / `ios-frame.jsx` / `android-frame.jsx` — 原型运行时与展示用设备外框，不需要实现。
