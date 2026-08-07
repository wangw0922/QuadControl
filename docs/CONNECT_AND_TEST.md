# 连接与测试

> **范围说明（2026-08-06）。** 本文描述的 Apple 连接诊断闭环**已不在产品路径上**。
> 它是为验证自研 Mac↔iPhone 链路而建的，而该链路已被架构调整移除：Mac→iPhone
> 改用 Apple iPhone Mirroring，Windows→iPhone 改用 go-ios + WebDriverAgent。
> 见 [控制架构决策](CONTROL_ARCHITECTURE.md)。
>
> 保留本文是因为这套诊断工具仍是仓库里唯一经过真机端到端验证的组件，可用于排查
> 局域网连通性。**不要把它当作产品能力。**

当前可运行的是 Apple **连接诊断闭环**：iPhone 主动连接 Mac，完成一次性 token 双向认证，并持续发送带 MAC 的心跳。它还不是投屏或控制产品，不包含画面、触控、键盘、剪贴板、文件、远程中继或熄屏控制。

## 1. 先在 Mac 做自动化回环测试（不需要任何手机）

在项目根目录运行：

```sh
scripts/build-macos.sh
swift test
swift run QuadControlSelfTest
```

预期 `swift test` 执行 4 个 XCTest 用例，`QuadControlSelfTest` 最后一行是：

```text
PASS: QuadControlSelfTest (52 assertions)
```

self-test 通过真实 `Network.framework` TCP 回环验证 framing、错误输入、HMAC/HKDF 固定向量、错误/过期/重放 token、限速、双向认证、heartbeat/ack，以及监听端在**显式端口**上的绑定（listener 实际使用的就是显式端口，早期只测过临时端口）。

若你的环境只有 Command Line Tools（`xcode-select -p` 指向 `CommandLineTools`），`swift test` 会因缺少 XCTest 而构建出空测试模块并退出 0 —— 那种情况下不能算测试通过，需要装完整 Xcode。SwiftPM 若被外层沙箱拒绝，可临时加 `--disable-sandbox`。

## 2. 不用真机：先在 iOS 模拟器跑通整条链路

这是最快的验证方式，只需要完整 Xcode，不需要签名、真机或同一局域网。模拟器与
Mac 共用网络栈，所以直接用 `127.0.0.1` 连本机 loopback listener。

```sh
brew install xcodegen
cd apps/ios && xcodegen generate --spec project.yml
xcodebuild -project QuadControlIOS.xcodeproj -scheme QuadControlIOS \
  -destination 'platform=iOS Simulator,name=iPhone 17' \
  CODE_SIGNING_ALLOWED=NO test
```

预期 `** TEST SUCCEEDED **`，包含 2 个 `ConnectionModelTests`。`name=iPhone 17`
换成 `xcrun simctl list devices available` 里你实际有的机型。

然后在**交互式 Terminal** 启动 loopback listener（不加 `--lan`）：

```sh
swift run QuadControlMacListener --port 47100
```

在模拟器里启动 app，填 `127.0.0.1` / `47100` / 终端显示的 43 字符 token，点
`Connect diagnostic session`。预期 iPhone 显示 `Connected; authenticated
heartbeats active`，Mac 侧依次出现 `handshake`、`authenticated`、`heartbeat 1`
并每 5 秒递增。

模拟器验证不能替代真机：它不经过 Wi-Fi 局域网路由，也不会弹出 iOS 的本地网络
权限授权框。这两项只有真机能证明。

## 3. 准备 iPhone 真机工程

需要完整 Xcode、XcodeGen、Apple ID/开发签名，以及一台由用户解锁并信任此 Mac 的 iPhone。若 `xcode-select -p` 仍指向 Command Line Tools，先在 Xcode 设置中安装所需组件，并选择完整 Xcode Developer 目录。

```sh
cd apps/ios
xcodegen generate --spec project.yml
open QuadControlIOS.xcodeproj
```

也可先回到项目根目录执行 `scripts/build-ios.sh`，做不签名的 generic iOS Simulator 编译检查。它只验证构建，不代表已运行 Simulator 或真机。

在 Xcode 中：

1. 选择 `QuadControlIOS` target 的 Signing & Capabilities。
2. 选择自己的 Team；若 bundle ID 冲突，改成自己的唯一 ID。
3. 用 USB 连接并解锁 iPhone，确认“信任此电脑”；系统要求时开启 Developer Mode。
4. 把运行目标切换为这台 iPhone，点击 Run。
5. 首次启动时允许“本地网络”权限。
6. 运行 Product > Test；应发现并通过 2 个 `ConnectionModelTests`。

本仓库没有提交生成的 `.xcodeproj`，也没有保存个人签名信息。模拟器构建与测试已在本机验证；**真机签名与 on-device 运行仍未在此机验证**。

## 4. 在 Mac 启动局域网监听

先确认 Mac 与 iPhone 在同一个可信局域网，且路由器没有开启客户端隔离。查找 Wi-Fi 对应的设备名和数字私网地址：

```sh
networksetup -listallhardwareports
ipconfig getifaddr en0
```

`en0` 只是常见值；请使用上一条命令显示的 Wi-Fi Device。然后在项目根目录的**交互式 Terminal**启动：

```sh
swift run QuadControlMacListener --lan --interface wifi --port 47100
```

有线网络改用 `--interface wired`。程序会在 `/dev/tty` 显示一次**实际的私网地址**、端口、43 字符临时 token 和一个包含这三项的二维码；token 180 秒过期。非交互环境会直接拒绝启动，以免 token 被 stdout 重定向保存。

界面语言跟随系统：中文系统显示中文。stderr 的日志行（`handshake`、`authenticated`、`heartbeat N`）固定英文，它们是给排查和测试匹配用的标识。

不要把 token 放进命令行、环境变量、文件、URL、剪贴板、聊天、日志或截图。监听端不接受公网 peer，客户端也只接受数字形式的 loopback、RFC1918、ULA 或 link-local 地址；不支持域名、VPN、NAT 穿透、Bonjour 或 relay。

## 5. 在 iPhone 发起连接

### 推荐：扫码

listener 就绪时会在终端直接显示一个二维码，里面是地址、端口和 token。在 app 里点
「扫描配对码」，对准终端上的二维码即可，不用手输 43 位 token。首次会请求相机权限。

二维码必须在**真正的终端**里看 —— 它用 ANSI 背景色保证黑白对比，重定向到文件或
贴进聊天窗口会丢掉颜色、可能反色导致扫不出来。

扫到的内容按不可信输入处理：地址仍要通过私网校验（公网地址、域名一律拒绝），token
仍要通过同样的解码校验。所以别人给的二维码不可能把你的手机指向公网监听端。

### 手动输入

也可以在 `QuadControl` 中手动输入：

- `Private host`：上一步得到的 Mac 数字私网 IP，例如 `192.168.1.23`；
- `Port`：默认 `47100`；
- `Temporary token`：Mac 终端刚显示的 43 字符值。

点击 `Connect diagnostic session`。token 会在提交、成功、失败、超时、进入后台或 Disconnect 时从输入状态清空。

成功时：

- iPhone 显示 `Connected; authenticated heartbeats active`；
- Mac stderr 依次出现 `handshake`、`authenticated`、`heartbeat 1`，随后约每 5 秒递增；
- 日志只显示随机短连接 ID，不显示 token、IP 或消息内容。

点击 Disconnect 或让 app 进入后台应立即断开。token 成功使用后已经消费；再次连接必须停止并重启 Mac listener，取得新 token。

## 6. iPhone 常见失败

iPhone 只会显示两类结果：本地输入校验失败，或 `Connection failed (network)`。
**服务端拒绝握手时不会回结构化错误码**（不向未认证的对端透露 token 是错、过期
还是已用过），所以真正的原因只能看 Mac listener 的 stderr。

- iPhone 提示 token 必须是 43 字符：本地长度/字符集校验没过，token 没输全。
- iPhone 显示 `Connection failed (network)`，Mac 日志显示 `closed authentication`
  / `closed expired` / `closed consumed`：分别是 token 错误、超过 180 秒 TTL、
  已经成功使用过。重启 listener 取新 token。
- iPhone 显示 `Connection failed (network)` 但 Mac 日志**没有任何新连接**：包没到
  Mac。检查两端是否同一局域网、本地网络权限、Mac 防火墙、IP/端口和路由器客户端
  隔离。首次权限弹窗处理超过 10 秒时，允许后用新 listener/token 重试。
- 若曾拒绝本地网络权限，在 iPhone 的 Settings > Privacy & Security > Local Network 中重新允许此 app。
- 输入域名或公网 IP 时 setup 失败：这是本阶段的主动安全限制，不是 DNS 故障。
- Mac 没显示 token：必须从真正的交互式 Terminal 启动，不能通过重定向、后台 daemon 或无人值守方式运行。

## 7. Android 手机目前只能测试 ADB 就绪度

Android 客户端和投屏/控制通道尚未实现，因此 Android **不能连接上述 Apple 诊断 listener**。现有 `quadcontrol-adb-probe` 只做只读准备检查。

1. 安装 Android Platform Tools 和 Rust。
2. 在手机打开开发者选项与 USB 调试，用 USB 连接并在手机上明确批准这台电脑。
3. 先确认 `adb devices -l` 显示状态 `device`，而不是 `unauthorized` 或 `offline`。
4. 在项目根目录运行：

```sh
cargo run -p quadcontrol-adb-probe -- --format json
```

它只执行 `adb version`、`adb devices -l`、`adb mdns services`，不会自动 pair、connect、执行 shell 或修改设备。当前开发机缺少 Cargo 和 adb，因此 Rust build/tests 与 Android 真机输出仍是 **Blocked / 未在此机验证**。
