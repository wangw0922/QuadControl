# Windows、macOS、Android、iPhone 四端通用控制软件制作计划

## 一、先明确项目能做到什么

这个项目可以启动，但必须把“熄屏”和“锁屏”分开定义，否则 GPT-5.6 很容易写出看似完整、实际无法运行的方案。

| 状态 | 准确定义 | Android | iPhone |
|---|---|---:|---:|
| 画面变黑 | 亮度降至最低或覆盖黑色界面，屏幕面板仍工作 | 可实现 | 可实现 |
| 物理熄屏 | 屏幕面板关闭，但系统仍处于已解锁、活跃状态 | 可实现 | 有条件，需要先验证 |
| 系统锁屏 | 出现密码、Face ID、Touch ID或锁屏界面 | 不保证继续控制 | 第三方公开API无法承诺 |
| 完整控制 | 点击、滑动、长按、输入文字、返回、主页、切换App | 可实现 | 有条件，需要输入桥接方案 |

Apple 自带的“iPhone镜像”确实可以在iPhone保持锁定时从Mac进行操作，但这是Apple自己的系统功能。Apple要求App Store应用只能使用公开API；ReplayKit公开提供的是屏幕录制和广播，并且需要用户通过系统广播选择器主动启动。在Apple当前公开文档中，没有向第三方开放“向其他App全局注入触控事件”的接口。因此，**iPhone真正锁屏后由Windows或自研Mac软件完整控制，不能作为普通第三方软件的固定交付承诺**，只能作为“申请Apple授权、受限权限或系统合作”的独立路线。

不过，iPhone还有一个可以研究的路径：VoiceOver的“屏幕帘幕”。Apple明确说明，开启屏幕帘幕后，iPhone仍然处于活跃状态，但显示屏会关闭；AssistiveTouch也支持通过蓝牙或USB鼠标、触控板等指针设备控制iPhone。理论上可以组合成：

> ReplayKit传输画面 + Screen Curtain关闭实体屏幕 + 电脑模拟外接鼠标键盘控制iPhone

但Apple没有说明ReplayKit在Screen Curtain开启后是否仍会传出正常画面，也没有保证Windows或Mac能够纯软件模拟成iPhone接受的蓝牙HID鼠标键盘，因此这必须作为第一阶段的硬性可行性实验。

Android路线相对明确。普通Android应用使用MediaProjection时，系统锁屏会终止屏幕捕获；但scrcpy已经证明，通过ADB和设备端临时服务，可以在Windows、macOS上控制Android，并支持关闭Android实体屏幕、键鼠控制、音频和剪贴板。Android的AccessibilityService也公开支持点击、滑动和多点手势，ADB则支持USB和无线配对。

因此，建议把项目拆成以下三条路线：

1. **确定交付路线**：Windows、Mac完整控制Android，Android物理熄屏后继续控制。
2. **实验交付路线**：Windows、Mac控制iPhone，并尝试Screen Curtain熄屏控制。
3. **Apple合作路线**：iPhone真正锁屏后仍能完整控制。

---

# 二、产品总体定义

暂定产品名：**QuadControl**

四个独立安装包：

| 平台 | 软件名称 | 角色 |
|---|---|---|
| Windows | QuadControl Desktop for Windows | 电脑控制端 |
| macOS | QuadControl Desktop for Mac | 电脑控制端 |
| Android | QuadControl Agent for Android | 手机被控端及权限管理 |
| iOS | QuadControl Agent for iPhone | 屏幕广播、连接管理及熄屏实验 |

产品第一阶段只定义为：

> Windows或Mac控制Android或iPhone。

暂不把“手机控制电脑”“Android控制iPhone”“iPhone控制Android”加入首版，避免架构和权限范围失控。

## 完整控制的范围

允许实现：

- 单击、双击、长按；
- 单指和多指滑动；
- 鼠标滚轮；
- 键盘文字输入；
- 中英文输入；
- 返回、主页、最近任务；
- 音量调整；
- 横竖屏自动适配；
- 复制粘贴；
- 手机声音传输到电脑；
- 截图和录像；
- USB连接和局域网无线连接；
- 用户主动授权后的远程网络连接；
- 关闭手机实体屏幕后继续使用。

明确排除：

- 绕过锁屏密码；
- 远程输入Face ID、Touch ID和设备密码；
- 绕过银行、密码管理器、DRM视频等安全页面；
- 隐藏运行、秘密监控；
- 未经手机用户确认的无人值守控制；
- 破解、越狱和Root作为常规要求。

Android应用可以通过`FLAG_SECURE`阻止截图和屏幕共享，因此部分银行、密码管理和流媒体页面会显示黑屏，这应当作为正常安全行为保留，不能尝试绕过。

---

# 三、总体技术架构

```text
┌─────────────────────────┐        ┌─────────────────────────┐
│ Windows Desktop         │        │ macOS Desktop           │
│ WinUI 3 + Rust Core     │        │ SwiftUI + Rust Core     │
└────────────┬────────────┘        └────────────┬────────────┘
             │                                  │
             └────────── 加密会话协议 ──────────┘
                              │
             ┌────────────────┴────────────────┐
             │                                 │
┌────────────▼────────────┐       ┌────────────▼────────────┐
│ Android Agent          │       │ iOS Agent               │
│ Kotlin + Shell Server  │       │ SwiftUI                 │
│ MediaCodec / ADB       │       │ ReplayKit Extension     │
│ AccessibilityService   │       │ Screen Curtain实验      │
└─────────────────────────┘       └─────────────────────────┘
```

## 推荐技术栈

| 模块 | 技术选择 |
|---|---|
| 跨平台核心 | Rust |
| 协议定义 | Protocol Buffers |
| 控制指令 | QUIC可靠流 |
| 视频和音频 | QUIC Datagram或独立低延迟媒体通道 |
| 视频编码 | H.264为基础，H.265可选 |
| 音频编码 | Opus |
| Windows界面 | C#、WinUI 3 |
| Windows渲染 | Direct3D 11/12、Media Foundation |
| macOS界面 | SwiftUI + AppKit |
| macOS渲染 | Metal + VideoToolbox |
| Android | Kotlin、Jetpack Compose、MediaCodec |
| iOS | Swift、SwiftUI、ReplayKit Broadcast Upload Extension |
| 局域网发现 | mDNS/Bonjour |
| 本地数据存储 | SQLite |
| 密钥存储 | Windows DPAPI、macOS Keychain、Android Keystore、iOS Keychain |
| 自动化测试 | Rust Test、JUnit、XCTest、Windows UI Automation |
| CI | GitHub Actions自托管Mac和Windows节点 |

Rust核心负责：

- 配对；
- 会话状态；
- 加密；
- 网络传输；
- 协议编解码；
- 视频帧排序；
- 输入事件标准化；
- 剪贴板和文件传输；
- 日志和诊断；
- 自动重连。

四个平台只实现各自的界面、系统权限、编码解码和设备连接能力，避免分别维护四套协议代码。

---

# 四、四个平台的具体制作方案

## 1. Windows控制端

### 核心功能

- 自动发现Android和iPhone；
- 通过USB识别Android设备；
- 管理ADB USB及无线配对；
- 扫描局域网内的iOS ReplayKit广播；
- 显示手机实时画面；
- 将鼠标坐标转换成手机标准化坐标；
- 传递键盘、触摸、系统按键；
- 多窗口控制多台手机；
- Android实体屏幕关闭开关；
- 设备横竖屏自动旋转；
- 剪贴板同步；
- 文件拖放；
- 声音播放；
- 录像和截图；
- 断线自动恢复；
- 托盘图标和紧急断开按钮。

### Windows端模块

```text
windows-app/
├── DeviceDiscovery
├── AdbManager
├── AndroidSession
├── IOSBroadcastReceiver
├── VideoRenderer
├── AudioRenderer
├── InputMapper
├── ClipboardBridge
├── FileTransfer
├── SessionSecurity
└── UpdateManager
```

### iPhone输入实验

Windows需要尝试将电脑模拟为iPhone可识别的蓝牙鼠标或键盘。但是Windows的Bluetooth Peripheral Role依赖具体硬件，不能假定所有Windows电脑都支持。Windows公开的GATT Server API可以创建BLE服务，但“能否作为完整HID over GATT鼠标键盘”必须在真实蓝牙芯片上单独验证。

因此Windows版要设计三种输入后端：

1. Android ADB输入后端；
2. Android Accessibility输入后端；
3. iPhone HID实验后端。

iPhone HID实验失败时，Windows版只能先提供iPhone屏幕查看，或者增加一个外接USB蓝牙HID小硬件作为后续备选。

---

## 2. macOS控制端

Mac端功能与Windows保持一致，但使用原生SwiftUI、VideoToolbox和Metal。

主要模块：

```text
macos-app/
├── DeviceBrowser
├── AdbProcessManager
├── AndroidSession
├── IOSBroadcastReceiver
├── MetalVideoRenderer
├── InputCoordinator
├── ClipboardCoordinator
├── FileTransfer
├── KeychainIdentityStore
└── UpdateManager
```

macOS的CoreBluetooth公开支持通过`CBPeripheralManager`发布GATT服务，但公开文档并没有保证普通Mac应用可以直接把整台Mac模拟成iPhone接受的系统级HID鼠标键盘。因此Mac到iPhone的纯软件输入同样必须经过P0实验，不能因为Mac和iPhone都属于Apple设备就默认可行。

Mac版不应尝试调用、控制或逆向Apple自带的iPhone镜像程序。可以在产品界面提示用户：需要真正锁屏控制时使用Apple原生iPhone镜像，但这不是自研软件功能。

---

## 3. Android被控端

Android端建议提供两个运行模式。

### 模式A：商店标准模式

技术：

- MediaProjection捕获屏幕；
- MediaCodec编码H.264；
- AccessibilityService执行点击和滑动；
- 自定义输入法处理中文、英文及特殊按键；
- 前台服务显示持续通知；
- 用户每次主动确认屏幕捕获；
- 手机保持正常亮屏或最低亮度。

限制：

- Android锁屏后MediaProjection会终止；
- 无法保证真正物理熄屏；
- 部分安全页面显示黑屏；
- AccessibilityService必须经过Google Play声明和显著授权提示。

Google Play允许非辅助工具使用AccessibilityService，但要求明确披露访问了什么数据、如何使用，并取得用户主动同意；自动化行为必须是狭窄、明确、由用户指令触发的确定性行为，不能自主规划并执行操作。

### 模式B：桌面增强模式

这是满足Android熄屏控制的主要方案。

工作流程：

1. 用户在Android打开开发者选项；
2. 开启USB调试或无线调试；
3. Windows或Mac通过二维码、配对码或USB获得授权；
4. 电脑临时推送Android Shell Server；
5. Shell Server捕获并编码画面；
6. 电脑通过控制通道发送触摸和键盘事件；
7. 用户点击“关闭手机屏幕”；
8. 手机实体屏幕关闭，但逻辑显示和远程会话继续；
9. 会话结束后恢复原始亮屏、旋转和超时设置。

建议基于scrcpy进行二次开发，而不是从零重写全部Android内部兼容逻辑。scrcpy官方项目已支持Windows、macOS、USB、TCP/IP、键鼠控制、Android实体屏幕关闭、HID输入和音频，并采用Apache 2.0许可证。商用时需要保留版权、许可证和修改说明。

Android应用本身负责：

- 引导开启权限；
- 显示当前配对电脑；
- 撤销授权；
- 连接状态；
- 安全提示；
- 标准模式；
- 恢复屏幕；
- 错误诊断。

真正的增强控制服务由电脑通过ADB临时启动，断开后不长期驻留。

---

## 4. iPhone被控端

iOS应用由两个Target组成：

```text
ios/
├── QuadControlApp
└── QuadControlBroadcastExtension
```

### iPhone主应用

负责：

- 显示Windows或Mac配对二维码；
- 保存可信电脑公钥；
- 检查局域网；
- 启动ReplayKit系统广播选择器；
- 展示连接状态；
- 指导用户设置VoiceOver、Screen Curtain和AssistiveTouch；
- 撤销电脑；
- 停止广播；
- 输出诊断报告。

### ReplayKit Broadcast Upload Extension

负责：

- 接收ReplayKit的屏幕帧；
- 转换方向、时间戳和分辨率；
- 使用VideoToolbox编码；
- 通过加密连接发送到电脑；
- 处理网络抖动；
- 控制码率；
- 在内存不足时降低分辨率；
- 连接终止后立即释放资源。

用户必须明确启动广播；Apple还要求屏幕录制具有明确同意和可见或可听提示。不能设计隐藏式自动录屏。

### iPhone熄屏模式

建议名称：

> Accessibility Screen-Off Mode  
> 辅助功能熄屏模式

用户操作流程：

1. 在iPhone中开启VoiceOver；
2. 开启AssistiveTouch；
3. 将电脑提供的HID设备配对为鼠标和键盘；
4. 在QuadControl中启动ReplayKit广播；
5. 确认电脑已经显示正常画面；
6. 使用三指三击开启Screen Curtain；
7. iPhone实体显示屏关闭；
8. 电脑继续接收画面并发送鼠标键盘输入；
9. 会话结束后关闭Screen Curtain和VoiceOver。

这一功能只能在下面的P0实验全部通过后加入正式版本。

---

# 五、iPhone必须先完成的P0可行性实验

| 实验 | 通过标准 | 失败后的处理 |
|---|---|---|
| ReplayKit + Screen Curtain | 开启Screen Curtain后，电脑仍连续收到真实画面30分钟，不是黑屏或静止帧 | iPhone只提供亮屏投屏 |
| Windows软件HID | 不加外部硬件，可在iPhone完成点击、拖动、滚动、文字输入和主页导航 | Windows版iPhone仅查看，或增加硬件桥 |
| Mac软件HID | 不依赖Apple原生iPhone镜像，可在iPhone全局导航 | Mac版iPhone仅查看，或使用原生镜像 |
| VoiceOver兼容 | VoiceOver开启后不会破坏鼠标坐标和键盘输入 | 改为键盘焦点导航模式 |
| 横竖屏 | 旋转后坐标自动重新映射 | 暂停输入并要求重新校准 |
| 长时运行 | 连续60分钟无扩展崩溃、无严重发热和不同步 | 降帧率、码率和分辨率 |
| App Store预审 | 仅使用公开API，功能说明和隐私披露明确 | iOS版缩减为投屏和辅助引导 |
| 锁屏测试 | 锁屏后会安全停止，而不是尝试绕过锁屏 | 保持该安全行为 |

这里最重要的停项条件是：

> 如果ReplayKit在Screen Curtain下只能输出黑屏，或者Windows与Mac都无法纯软件模拟iPhone接受的HID设备，就无法完成“纯软件iPhone熄屏完整控制”。

此时仍然可以发布：

- Android完整控制版；
- iPhone亮屏投屏版；
- Mac用户调用Apple原生iPhone镜像；
- 搭配一个专用USB/Bluetooth HID硬件的iPhone增强版。

---

# 六、连接协议设计

## 会话生命周期

```text
未配对
  ↓
二维码或PIN配对
  ↓
交换设备身份公钥
  ↓
权限检查
  ↓
能力协商
  ↓
视频和音频通道建立
  ↓
输入控制开启
  ↓
可选实体熄屏
  ↓
断开或异常恢复
  ↓
恢复手机原始显示状态
```

## 协议消息

```text
PairRequest
PairResponse
CapabilityReport
SessionOffer
SessionAccept
VideoConfig
VideoFrame
AudioConfig
AudioFrame
PointerEvent
TouchEvent
KeyEvent
TextInput
SystemAction
ClipboardEvent
FileMetadata
FileChunk
OrientationChanged
DisplayStateChanged
PermissionChanged
Heartbeat
ErrorReport
SessionEnd
```

## 输入坐标

所有触控坐标使用标准化数值：

```text
x: 0.0 ～ 1.0
y: 0.0 ～ 1.0
```

手机端根据：

- 实际分辨率；
- 横竖屏；
- 刘海和动态岛安全区域；
- 导航栏；
- 显示缩放；
- 折叠屏状态；

转换成设备真实坐标。

## 通道优先级

1. 紧急断开和安全指令；
2. 鼠标、触摸和键盘；
3. 音频；
4. 视频关键帧；
5. 普通视频帧；
6. 剪贴板；
7. 文件传输和日志。

文件传输不能抢占输入事件和视频带宽。

---

# 七、安全和隐私设计

这是远程控制软件，安全要求必须从第一天加入，而不是最后补充。

### 配对安全

- 手机必须在现场显示二维码或六位配对码；
- 首次配对必须在手机端主动确认；
- 使用临时密钥协商；
- 每台设备生成长期身份密钥；
- 可信电脑列表可随时撤销；
- 配对码短时间失效；
- 防止重放攻击。

### 会话安全

- 所有画面、输入、音频和文件端到端加密；
- 输入事件带有递增序号和时间戳；
- 默认仅允许局域网连接；
- 远程中继必须单独开启；
- 中继服务器不得持有解密密钥；
- 同一时间默认只允许一个控制端；
- 每次控制都显示手机端和电脑端状态指示；
- 提供电脑快捷键和手机按钮立即断开。

### 隐私限制

- 不记录密码框文字；
- 不保存屏幕视频，除非用户主动点击录像；
- 剪贴板同步默认关闭；
- 日志不包含屏幕内容和输入文字；
- 受保护页面显示黑屏；
- 锁屏后停止iOS会话；
- 不提供隐形运行；
- 不提供默认无人值守访问；
- 不尝试控制Face ID、Touch ID和支付确认。

---

# 八、代码仓库结构

```text
quadcontrol/
├── AGENTS.md
├── README.md
├── LICENSES/
│   ├── THIRD_PARTY_NOTICES.md
│   └── SCRCPY_NOTICE.md
├── docs/
│   ├── PRODUCT_REQUIREMENTS.md
│   ├── PRODUCT_CONSTRAINTS.md
│   ├── ARCHITECTURE.md
│   ├── PROTOCOL.md
│   ├── SECURITY.md
│   ├── THREAT_MODEL.md
│   ├── IOS_FEASIBILITY.md
│   ├── ANDROID_COMPATIBILITY.md
│   ├── DECISIONS.md
│   ├── RISKS.md
│   └── STATUS.md
├── apps/
│   ├── windows/
│   ├── macos/
│   ├── android/
│   └── ios/
├── extensions/
│   └── ios-broadcast/
├── agents/
│   └── android-shell/
├── crates/
│   ├── core/
│   ├── protocol/
│   ├── transport/
│   ├── crypto/
│   ├── media/
│   ├── pairing/
│   ├── diagnostics/
│   └── ffi/
├── services/
│   ├── signaling/
│   └── relay/
├── tests/
│   ├── protocol/
│   ├── integration/
│   ├── security/
│   └── device-lab/
└── scripts/
    ├── build-windows.ps1
    ├── build-macos.sh
    ├── build-android.sh
    ├── build-ios.sh
    └── verify-all.sh
```

## 功能开关

```text
android_standard
android_adb_full
android_screen_off
ios_view
ios_replaykit
ios_screen_curtain_experimental
ios_hid_windows_experimental
ios_hid_macos_experimental
remote_relay
file_transfer
audio_forwarding
```

实验功能不能在验证完成前默认开启。

---

# 九、开发阶段和排期

以下是约6人并行开发的建议排期，不是单次生成代码的时间承诺。

| 阶段 | 周期 | 主要交付 |
|---|---:|---|
| M0 可行性验证 | 第1～2周 | Android熄屏PoC、iOS Screen Curtain、Windows/Mac HID实验 |
| M1 基础架构 | 第3～5周 | Monorepo、Rust核心、协议、配对、加密、CI |
| M2 Windows + Android纵向闭环 | 第6～9周 | USB/Wi-Fi、画面、输入、Android熄屏 |
| M3 macOS + Android | 第10～12周 | Mac端完整Android控制 |
| M4 iOS投屏和实验控制 | 第13～16周 | ReplayKit、iOS连接、Screen Curtain、HID输入 |
| M5 产品功能 | 第17～18周 | 音频、剪贴板、文件、录像、自动重连 |
| M6 QA和发布 | 第19～20周 | 安全审计、设备兼容、安装包、商店材料 |

## 推荐人员配置

| 人员 | 职责 |
|---|---|
| 技术负责人/Rust工程师 | 架构、协议、加密、共享核心 |
| Windows工程师 | WinUI、D3D、ADB、安装包 |
| macOS工程师 | SwiftUI、Metal、ADB、签名公证 |
| Android工程师 | Agent、ADB Server、权限和OEM兼容 |
| iOS工程师 | ReplayKit、扩展、Screen Curtain实验 |
| QA/安全工程师 | 真机矩阵、自动化、威胁建模、发布审查 |

GPT-5.6可以负责大部分代码生成、测试设计、协议实现和问题排查，但以下环节仍必须由真人在真实设备上完成：

- Apple证书和签名；
- iPhone ReplayKit实验；
- Bluetooth HID配对；
- Windows不同蓝牙芯片测试；
- Samsung、小米、OPPO、vivo等OEM兼容；
- App Store和Google Play提交；
- 安全审计。

---

# 十、测试矩阵与验收标准

## 设备矩阵

Android至少覆盖：

- Google Pixel；
- Samsung Galaxy；
- Xiaomi/Redmi；
- OPPO/OnePlus；
- vivo；
- Motorola；
- 不同Android大版本；
- USB调试和无线调试；
- 手势导航和三键导航。

iPhone至少覆盖：

- 最低支持版本；
- 当前主要版本；
- 普通屏幕和动态岛机型；
- Lightning和USB-C机型；
- VoiceOver打开和关闭；
- AssistiveTouch打开和关闭；
- Wi-Fi、蓝牙和蜂窝网络组合。

电脑至少覆盖：

- Windows 11 x64；
- Windows 11 ARM64；
- Intel Mac；
- Apple Silicon Mac；
- 不同品牌Bluetooth适配器；
- 2.4GHz、5GHz和Wi-Fi 6网络。

## MVP验收指标

| 项目 | 验收标准 |
|---|---|
| 基础视频 | 1080p、30fps稳定运行 |
| Android目标视频 | 支持60fps设备达到1080p 60fps |
| 局域网输入延迟 | P95不高于100ms |
| USB输入延迟 | P95不高于60ms |
| 自动重连 | 网络短断后5秒内恢复 |
| Android熄屏 | 实体屏幕关闭后连续控制60分钟 |
| Android旋转 | 横竖屏切换后坐标自动正确 |
| iOS投屏 | 连续运行60分钟无扩展崩溃 |
| iOS实验熄屏 | Screen Curtain开启后仍传真实画面 |
| 会话结束 | 自动恢复原亮度、旋转和屏幕状态 |
| 安全 | 未配对电脑不能建立控制连接 |
| 锁屏保护 | 不得绕过锁屏、密码和生物识别 |
| 安全页面 | 受保护内容保持黑屏 |
| 崩溃恢复 | 手机端和电脑端都不会留下失控后台进程 |

---

# 十一、发布方案

## Windows

- MSIX安装包；
- 独立签名安装程序作为备用；
- x64和ARM64；
- 自动更新包必须签名；
- Android ADB组件随安装包一起审核许可证。

## macOS

- Apple Developer ID签名；
- 公证后的DMG；
- Apple Silicon和Intel版本；
- 首版优先官网直接分发；
- 不把需要额外进程、ADB和实验蓝牙能力的版本强行塞入Mac App Store沙盒。

## Android

建议两个渠道：

### Google Play标准版

- MediaProjection；
- AccessibilityService；
- 显著披露；
- 用户主动授权；
- 不承诺锁屏或真正熄屏；
- 所有操作由用户实时输入触发。

### 官网增强版

- 与Windows/Mac ADB增强模式配合；
- 支持实体屏幕关闭；
- 明确要求开启开发者选项；
- 用户可随时撤销USB和无线调试授权。

## iOS

- App Store版本只使用公开API；
- ReplayKit必须由用户启动；
- Screen Curtain只做用户引导，不尝试私有调用；
- 实验功能使用远程配置关闭开关；
- 不宣传“锁屏控制”，除非Apple书面确认并提供合规能力。

---

# 十二、直接复制给GPT-5.6的项目总提示词

```text
你是本项目的首席软件架构师和高级跨平台工程师。你的任务是实际创建并持续维护一个名为 QuadControl 的四端项目，而不是只提供示例代码。

# 一、产品目标

创建四个软件：

1. Windows控制端
2. macOS控制端
3. Android被控端
4. iOS被控端

Windows和macOS负责显示并控制Android或iPhone。

核心目标：
- Android实体屏幕关闭后，Windows和Mac仍能显示并完整控制Android。
- iPhone优先实现ReplayKit投屏。
- iPhone的Screen Curtain熄屏控制属于实验功能，只有真实设备验证通过后才能标记完成。
- iPhone真正系统锁屏后的第三方完整控制不属于MVP，不得虚构可用API或使用私有API实现。

# 二、不可违反的事实约束

1. 区分Screen Off和Device Locked。
2. Android普通MediaProjection在锁屏后会停止。
3. Android增强模式使用ADB和临时Shell Server。
4. Android增强控制优先基于scrcpy的合法Apache 2.0代码和思路开发。
5. 必须保留所有第三方许可证和修改说明。
6. iOS只使用Apple公开API。
7. iOS屏幕传输使用ReplayKit Broadcast Upload Extension。
8. iOS广播必须由用户主动启动。
9. 不得使用私有API、越狱、Root、锁屏绕过或密码注入。
10. iOS全局输入只能通过真实验证过的外部HID或Apple正式授权能力实现。
11. 如果Screen Curtain下ReplayKit返回黑屏，必须把该实验标记为Blocked，不能伪造实现。
12. 不控制Face ID、Touch ID、系统密码、支付确认和安全页面。
13. 不实现隐藏控制和默认无人值守访问。

# 三、技术架构

- Monorepo
- Rust共享核心
- Protocol Buffers协议
- QUIC加密传输
- H.264基础视频编码
- Opus音频
- Windows：C# + WinUI 3 + Direct3D/Media Foundation
- macOS：SwiftUI/AppKit + Metal + VideoToolbox
- Android：Kotlin + Jetpack Compose + MediaCodec
- iOS：SwiftUI + ReplayKit Broadcast Upload Extension
- 使用平台安全存储保存设备身份密钥
- 本地局域网优先
- 云端中继作为后续可选模块

# 四、代码仓库

创建以下目录：

apps/windows
apps/macos
apps/android
apps/ios
extensions/ios-broadcast
agents/android-shell
crates/core
crates/protocol
crates/transport
crates/crypto
crates/media
crates/pairing
crates/diagnostics
crates/ffi
services/signaling
services/relay
tests/protocol
tests/integration
tests/security
tests/device-lab
docs

必须创建并维护：

AGENTS.md
README.md
docs/PRODUCT_REQUIREMENTS.md
docs/PRODUCT_CONSTRAINTS.md
docs/ARCHITECTURE.md
docs/PROTOCOL.md
docs/SECURITY.md
docs/THREAT_MODEL.md
docs/IOS_FEASIBILITY.md
docs/ANDROID_COMPATIBILITY.md
docs/DECISIONS.md
docs/RISKS.md
docs/STATUS.md

# 五、执行顺序

严格按以下里程碑执行，不得同时铺开四端UI。

M0：可行性PoC
- Android ADB连接
- Android画面和控制
- Android实体屏幕关闭
- iOS ReplayKit广播
- iOS Screen Curtain下画面测试框架
- Windows Bluetooth HID能力检测
- macOS Bluetooth HID能力检测
- 输出可行性报告

M1：共享核心
- 配对
- 设备身份
- 加密
- 消息协议
- 会话状态机
- 心跳
- 自动重连
- 单元测试

M2：Windows + Android纵向闭环
- USB
- Wi-Fi
- 视频
- 鼠标
- 键盘
- 横竖屏
- Android熄屏
- 会话恢复

M3：macOS + Android
- 功能与Windows保持一致

M4：iOS
- ReplayKit Broadcast Upload Extension
- 画面传输
- Windows接收
- macOS接收
- Screen Curtain实验
- HID实验
- 对失败能力使用Blocked状态

M5：扩展功能
- 音频
- 剪贴板
- 文件
- 录像
- 截图
- 多设备

M6：发布
- 签名
- 安装包
- 第三方许可
- 隐私政策
- 商店说明
- 威胁模型
- 安全测试
- 设备兼容报告

# 六、工程要求

1. 每次只完成一个明确里程碑或子任务。
2. 修改前先检查现有仓库结构和代码风格。
3. 不创建重复协议或重复工具类。
4. 所有跨平台消息必须先写入.proto文件。
5. 所有新增功能必须有单元测试或集成测试。
6. 所有安全相关功能必须有失败测试。
7. 不允许硬编码密钥、密码、IP地址和证书。
8. 所有网络输入必须做长度和类型检查。
9. 视频帧、文件和日志必须设置最大尺寸。
10. 输入事件必须包含会话ID、递增序号和时间戳。
11. 断开后必须恢复手机原始屏幕状态。
12. 不允许因为一个平台失败而伪造另一个平台已完成。
13. iOS实验能力使用feature flag，默认关闭。
14. 构建结束后运行格式化、静态检查、单元测试和集成测试。
15. 检查git diff和git status，清理临时文件。
16. 更新docs/STATUS.md、docs/RISKS.md和docs/DECISIONS.md。

# 七、每次任务的输出格式

每次完成任务后只输出：

1. 完成内容
2. 修改文件
3. 构建命令
4. 测试命令
5. 测试结果
6. 尚未解决的问题
7. 风险变化
8. 下一项可执行任务

所有任务必须标记为：
- Done
- Blocked
- Cancelled

不得留下没有说明的Pending状态。

# 八、首个任务

现在只执行M0的第一部分：

1. 创建Monorepo基础目录。
2. 创建Rust workspace。
3. 创建协议、传输、加密和诊断空crate。
4. 创建Windows、macOS、Android、iOS最小可编译项目说明。
5. 创建全部核心文档。
6. 创建Android ADB能力探测命令行PoC。
7. 自动检测adb版本、已连接设备、USB连接和无线配对状态。
8. 编写单元测试。
9. 运行所有能够运行的构建和测试。
10. 不开始制作正式UI。
```

---

# 十三、最终推荐的产品发布路径

最稳妥的顺序是：

### 第一版

- Windows完整控制Android；
- Mac完整控制Android；
- Android实体屏幕关闭后继续控制；
- Windows和Mac查看iPhone屏幕；
- iPhone保持亮屏。

### 第二版

仅在P0实验通过后增加：

- iPhone Screen Curtain实体熄屏；
- Windows控制iPhone；
- Mac自研软件控制iPhone；
- iPhone外接HID输入。

### 第三版

- 跨网络远程连接；
- 多设备管理；
- 企业设备管理；
- 账号和可信设备同步；
- Apple正式授权路线。

因此，**这个项目可以立项，但首个里程碑必须是iPhone Screen Curtain、ReplayKit和电脑HID模拟的真实设备实验；不能先投入几个月完成四端界面，再发现iPhone底层权限根本不成立。**Android完整版本可以直接进入正式研发，iPhone真正锁屏控制必须独立列为Apple授权型目标。
