# 连接与测试

当前可运行的是 Apple **连接诊断闭环**：iPhone 主动连接 Mac，完成一次性 token 双向认证，并持续发送带 MAC 的心跳。它还不是投屏或控制产品，不包含画面、触控、键盘、剪贴板、文件、远程中继或熄屏控制。

## 1. 先在 Mac 做自动化回环测试

在项目根目录运行：

```sh
scripts/build-macos.sh
swift run --disable-sandbox QuadControlSelfTest
```

预期最后一行是：

```text
PASS: QuadControlSelfTest (51 assertions)
```

这会通过真实 `Network.framework` TCP 回环验证 framing、错误输入、HMAC/HKDF 固定向量、错误/过期/重放 token、限速、双向认证和 heartbeat/ack。当前 Command Line Tools 没有 XCTest，因此不能把空的 `swift test` 当成测试通过；完整 Xcode 环境再运行 XCTest。

## 2. 准备 iPhone 真机工程

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
6. 运行 Product > Test；当前应发现 2 个 `ConnectionModelTests`（本环境没有完整 Xcode，尚未实际执行）。

本仓库没有提交生成的 `.xcodeproj`，也没有保存个人签名信息。当前开发环境缺少完整 Xcode，所以 iOS build、Simulator、签名与真机运行仍是 **Blocked / 未在此机验证**。

## 3. 在 Mac 启动局域网监听

先确认 Mac 与 iPhone 在同一个可信局域网，且路由器没有开启客户端隔离。查找 Wi-Fi 对应的设备名和数字私网地址：

```sh
networksetup -listallhardwareports
ipconfig getifaddr en0
```

`en0` 只是常见值；请使用上一条命令显示的 Wi-Fi Device。然后在项目根目录的**交互式 Terminal**启动：

```sh
swift run QuadControlMacListener --lan --interface wifi --port 47100
```

有线网络改用 `--interface wired`。程序会在 `/dev/tty` 显示一次实际端口和 43 字符临时 token；token 180 秒过期。非交互环境会直接拒绝启动，以免 token 被 stdout 重定向保存。

不要把 token 放进命令行、环境变量、文件、URL、剪贴板、聊天、日志或截图。监听端不接受公网 peer，客户端也只接受数字形式的 loopback、RFC1918、ULA 或 link-local 地址；不支持域名、VPN、NAT 穿透、Bonjour 或 relay。

## 4. 在 iPhone 发起连接

在 `QuadControl Diagnostic` 中手动输入：

- `Private host`：上一步得到的 Mac 数字私网 IP，例如 `192.168.1.23`；
- `Port`：默认 `47100`；
- `Temporary token`：Mac 终端刚显示的 43 字符值。

点击 `Connect diagnostic session`。token 会在提交、成功、失败、超时、进入后台或 Disconnect 时从输入状态清空。

成功时：

- iPhone 显示 `Connected; authenticated heartbeats active`；
- Mac stderr 依次出现 `handshake`、`authenticated`、`heartbeat 1`，随后约每 5 秒递增；
- 日志只显示随机短连接 ID，不显示 token、IP 或消息内容。

点击 Disconnect 或让 app 进入后台应立即断开。token 成功使用后已经消费；再次连接必须停止并重启 Mac listener，取得新 token。

## 5. iPhone 常见失败

- `length`：token 不是完整的 43 字符值。
- `authentication`、`expired` 或 `consumed`：token 错误、超过 180 秒，或已经成功使用；重启 listener。
- `network` 或 `timeout`：检查两端是否同一局域网、本地网络权限、Mac 防火墙、IP/端口和路由器客户端隔离。首次权限弹窗处理超过 10 秒时，允许后用新 listener/token 重试。
- 若曾拒绝本地网络权限，在 iPhone 的 Settings > Privacy & Security > Local Network 中重新允许此 app。
- 输入域名或公网 IP 时 setup 失败：这是本阶段的主动安全限制，不是 DNS 故障。
- Mac 没显示 token：必须从真正的交互式 Terminal 启动，不能通过重定向、后台 daemon 或无人值守方式运行。

## 6. Android 手机目前只能测试 ADB 就绪度

Android 客户端和投屏/控制通道尚未实现，因此 Android **不能连接上述 Apple 诊断 listener**。现有 `quadcontrol-adb-probe` 只做只读准备检查。

1. 安装 Android Platform Tools 和 Rust。
2. 在手机打开开发者选项与 USB 调试，用 USB 连接并在手机上明确批准这台电脑。
3. 先确认 `adb devices -l` 显示状态 `device`，而不是 `unauthorized` 或 `offline`。
4. 在项目根目录运行：

```sh
cargo run -p quadcontrol-adb-probe -- --format json
```

它只执行 `adb version`、`adb devices -l`、`adb mdns services`，不会自动 pair、connect、执行 shell 或修改设备。当前开发机缺少 Cargo 和 adb，因此 Rust build/tests 与 Android 真机输出仍是 **Blocked / 未在此机验证**。
