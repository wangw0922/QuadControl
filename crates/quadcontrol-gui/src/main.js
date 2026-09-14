const messages = {
  zh: {
    appTitle: "QuadControl",
    myDevices: "我的设备",
    addDevice: "添加设备",
    android: "安卓",
    androidName: "Android",
    ios: "iPhone",
    iphone: "iPhone",
    controlling: "正在控制",
    available: "可连接",
    unauthorized: "未授权（需在手机上允许调试）",
    offline: "离线",
    pairing_required: "需要配对",
    other: "未知状态",
    noDevices: "未发现设备",
    err_adb_not_found: "没有找到 adb，请先安装 Android 平台工具后重试。",
    err_adb_timed_out: "adb 响应超时，请拔插数据线或重启 adb 后重试。",
    err_adb_failed: "读取安卓设备列表失败，请检查手机是否已开启 USB 调试。",
    err_ios_not_found: "没有找到 go-ios，iPhone 暂时无法列出。",
    err_ios_failed: "读取 iPhone 列表失败。",
    err_ios_version: "go-ios 版本过低或无法识别，请安装 1.2.1 及以上的正式版。",
    err_usbmuxd_unavailable: "连不上 usbmuxd，请确认 usbmuxd 服务正在运行、数据线已插好。",
    err_tunnel_port_busy: "go-ios 的隧道端口已被占用，请先结束另一个 go-ios 隧道。",
    err_tunnel_failed: "建立 iPhone 隧道失败，请拔插数据线后重试。",
    err_wda_not_installed: "手机上没有安装 WebDriverAgent，请先用你的开发者证书安装它。",
    err_wda_signature_expired: "WebDriverAgent 的签名已过期，请重新签名并安装后重试。",
    err_wda_unreachable: "WebDriverAgent 起来了但连不上，请在手机上确认它已允许运行。",
    err_wda_session_failed: "无法在 WebDriverAgent 上建立会话，请重新连接后重试。",
    err_forward_failed: "端口转发失败，请拔插数据线后重试。",
    err_proxy_failed: "画面转发中断，请重新连接。",
    err_device_locked: "手机已锁定，请在手机上解锁后继续。",
    err_text_unconfirmed: "无法确认已送达，请在手机上核对。",
    err_screenshot_failed: "截屏失败，请重试。",
    err_ios_session_already_running: "已有一个 iPhone 会话在进行中；同一时刻只能控制一台 iPhone。",
    err_ios_session_not_running: "iPhone 会话没有在运行，请先开始控制。",
    err_ios_process_error: "与 go-ios 通信失败，请查看日志后重试。",
    err_windows_session_unsupported: "Windows 上暂不支持开始会话（后续版本提供）。",
    err_macos_uses_iphone_mirroring: "在 Mac 上请使用系统自带的「iPhone 镜像」。",
    err_iphone_mirroring_failed: "无法打开「iPhone 镜像」，请在启动台里手动打开。",
    err_unsupported: "当前系统不支持这个操作。",
    iosStageHint: "iPhone 实时画面\n（点击画面 = 点手机屏幕）",
    iosConnected: "已连接 · {fps} 帧/秒",
    iosDisconnected: "未连接",
    iosLocked: "手机已锁定，请在手机上解锁后继续",
    iosSendTextLabel: "发送文字到 iPhone",
    iosSendTextPlaceholder: "在这里打字，直接输入到手机…",
    iosSend: "发送",
    iosTextSent: "已发送",
    iosWake: "唤醒屏幕",
    iosCapture: "截取屏幕",
    iosHome: "回到主屏幕",
    iosScreenshotSaved: "已保存到 {path}",
    iosSoundTitle: "声音保留在手机上",
    iosSoundDescription: "iPhone 的声音暂时无法转发到电脑，这是系统限制。",
    iosLimitsTitle: "iPhone 控制的已知限制",
    iosLimitsDescription:
      "控制走 Apple 的 UI 自动化，不是输入注入，个别应用可能不响应；" +
      "需要你自备开发者证书签名的 WebDriverAgent，签名过期后要重新安装；" +
      "手机屏幕的显示状态由手机自己决定，本软件不做任何控制；" +
      "手机锁定时画面为黑、文字不送达，请在手机上解锁。",
    iosMirroringTitle: "在 Mac 上用系统「iPhone 镜像」",
    iosMirroringDescription:
      "macOS 自带的「iPhone 镜像」体验更完整，QuadControl 直接帮你打开它，" +
      "之后的操作都在那个窗口里完成。QuadControl 不会驱动或自动化这个应用。",
    iosMirroringOpen: "打开 iPhone 镜像",
    iosMirroringUnavailable: "需要 macOS 15 及以上",
    iosSessionElsewhere: "已有 iPhone 会话在运行",
    err_ios_link_lost: "iPhone 链路已中断，请重新连接。",
    err_no_active_element: "手机上没有聚焦的输入框，请先在手机上点进一个输入框再发送。",
    err_unknown: "设备发现暂时不可用。",
    showMirror: "显示镜像窗口",
    disconnect: "断开连接",
    sessionStatus: "会话状态",
    sessionRunning: "会话进行中",
    sessionStarting: "正在启动会话…",
    sessionStopping: "正在断开连接…",
    sessionExited: "会话已退出（代码 {code}）",
    sessionFailed: "会话失败",
    futureVersion: "此功能将在未来版本提供",
    err_session_already_running: "这台设备已经有一个进行中的会话。",
    err_scrcpy_not_found: "没有找到 scrcpy，请先安装 scrcpy 4.1 或更高版本。",
    err_scrcpy_version: "scrcpy 版本不受支持，请安装稳定版 4.1 或更高版本。",
    err_session_start_failed: "会话启动失败，请检查设备连接后重试。",
    err_session_failed: "会话失败，请重新连接设备后重试。",
    mirrorDescription: "手机画面在独立的镜像窗口中显示\n（scrcpy 实时画面 · 可直接点按操作）",
    sessionControls: "会话控制",
    soundOutput: "声音输出",
    soundDescription: "手机的声音从哪里播放",
    thisComputer: "这台电脑",
    phone: "手机",
    screenOff: "手机熄屏",
    screenOffDescription: "熄灭手机屏幕，画面仍在电脑上显示",
    on: "开",
    off: "关",
    captureScreen: "截取手机屏幕",
    captureDescription: "保存到这台电脑的\"图片\"文件夹",
    capture: "截屏",
    safetyTitle: "连接由你发起，随时可断开",
    safetyDescription:
      "这台电脑已由你在手机上明确授权（开发者选项 + USB 调试）。会话随时可断开，" +
      "结束后会恢复手机的亮度、旋转与超时设置；本软件不会绕过锁屏或密码。",
    pairingDialog: "添加设备配对向导",
    chooseType: "选择类型",
    scanPair: "扫码配对",
    startControl: "开始控制",
    chooseDeviceTitle: "选择要配对的设备",
    chooseDeviceDescription: "选择要配对的 Android 手机。iPhone 暂不支持（后续版本）。",
    androidPairing: "使用 Android 系统的无线调试二维码配对",
    iphonePairing: "暂不支持（后续版本）",
    next: "下一步",
    scanTitle: "用手机系统设置扫描配对二维码",
    qrPlaceholder: "Android 无线调试配对二维码",
    qrCountdown: "等待手机扫码 · 剩余 {seconds}",
    scanStepOne: "在手机上打开「设置 → 开发者选项 → 无线调试」",
    scanStepTwo: "点击「使用二维码配对设备」",
    scanStepThree: "对准这张二维码即可；首次启用无线调试时，手机可能要求确认信任当前 Wi‑Fi 网络",
    networkNote: "这是 Android 无线调试的原生配对二维码，手机系统设置会读取它。",
    manualLink: "无法扫码？手动输入配对码",
    previous: "上一步",
    waitingScan: "等待手机扫码…",
    manualDescription: "输入手机上显示的配对地址、配对端口和 6 位配对码。配对成功后如需连接，请另行输入无线调试主页上的连接端口。",
    hostLabel: "手机上显示的配对地址",
    hostPlaceholder: "例如 192.168.1.20",
    portLabel: "手机上显示的配对端口",
    portPlaceholder: "1–65535",
    pairingCodeLabel: "手机上显示的 6 位配对码",
    pairingCodePlaceholder: "输入 6 位数字",
    continue: "继续",
    pairingCompleteTitle: "配对完成，可以开始控制",
    pairingCompleteDescription: "已配对设备：{device}",
    pairing_already_running: "已有配对流程正在进行。",
    manual_pair_needs_connect_port: "配对成功，但设备尚未上线。请输入无线调试主页上的连接端口。",
    pair_failed: "配对失败，请检查手机状态后重试。",
    pair_timed_out: "配对超时，请重新打开无线调试并重试。",
    pair_cancelled: "配对已取消。",
    done: "完成",
  },
  en: {
    appTitle: "QuadControl",
    myDevices: "My devices",
    addDevice: "Add device",
    android: "Android",
    androidName: "Android",
    ios: "iPhone",
    iphone: "iPhone",
    controlling: "Controlling",
    available: "Ready to connect",
    unauthorized: "Unauthorized (allow debugging on the phone)",
    offline: "Offline",
    pairing_required: "Pairing required",
    other: "Unknown status",
    noDevices: "No devices found",
    err_adb_not_found: "adb was not found. Install the Android platform tools and try again.",
    err_adb_timed_out: "adb stopped responding. Reconnect the cable or restart adb, then try again.",
    err_adb_failed: "Could not read the Android device list. Check that USB debugging is enabled.",
    err_ios_not_found: "go-ios was not found, so iPhones cannot be listed yet.",
    err_ios_failed: "Could not read the iPhone list.",
    err_ios_version: "This go-ios version is too old or unreadable. Install stable go-ios 1.2.1 or newer.",
    err_usbmuxd_unavailable: "usbmuxd is not reachable. Check that the usbmuxd service runs and the cable is connected.",
    err_tunnel_port_busy: "The go-ios tunnel port is already in use. Stop the other go-ios tunnel first.",
    err_tunnel_failed: "The iPhone tunnel could not be started. Reconnect the cable and try again.",
    err_wda_not_installed: "WebDriverAgent is not installed on the phone. Install it with your own developer certificate first.",
    err_wda_signature_expired: "The WebDriverAgent signature has expired. Re-sign and reinstall it, then try again.",
    err_wda_unreachable: "WebDriverAgent started but is unreachable. Allow it to run on the phone and try again.",
    err_wda_session_failed: "A WebDriverAgent session could not be created. Reconnect and try again.",
    err_forward_failed: "Port forwarding failed. Reconnect the cable and try again.",
    err_proxy_failed: "The live view stopped. Reconnect to continue.",
    err_device_locked: "The iPhone is locked. Unlock it on the phone to continue.",
    err_text_unconfirmed: "Delivery could not be confirmed — check the text on the iPhone.",
    err_screenshot_failed: "The screenshot could not be saved. Try again.",
    err_ios_session_already_running: "An iPhone session is already running; only one iPhone can be controlled at a time.",
    err_ios_session_not_running: "No iPhone session is running. Start control first.",
    err_ios_process_error: "Communication with go-ios failed. Check the logs and try again.",
    err_windows_session_unsupported: "Starting a session is not supported on Windows yet (coming later).",
    err_macos_uses_iphone_mirroring: "On a Mac, use Apple's own iPhone Mirroring.",
    err_iphone_mirroring_failed: "iPhone Mirroring could not be opened. Open it from Launchpad instead.",
    err_unsupported: "This operating system does not support that action.",
    iosStageHint: "iPhone live view\n(click the view = tap the phone)",
    iosConnected: "Connected · {fps} fps",
    iosDisconnected: "Not connected",
    iosLocked: "The phone is locked — unlock it on the phone to continue",
    iosSendTextLabel: "Send text to the iPhone",
    iosSendTextPlaceholder: "Type here to type on the phone…",
    iosSend: "Send",
    iosTextSent: "Sent",
    iosWake: "Wake the screen",
    iosCapture: "Capture screen",
    iosHome: "Go to the home screen",
    iosScreenshotSaved: "Saved to {path}",
    iosSoundTitle: "Sound stays on the phone",
    iosSoundDescription: "iPhone audio cannot be forwarded to this computer; that is a system limitation.",
    iosLimitsTitle: "Known limits of iPhone control",
    iosLimitsDescription:
      "Control goes through Apple's UI automation rather than input injection, so some apps may not respond; " +
      "you need your own developer-signed WebDriverAgent and must reinstall it when the signature expires; " +
      "the phone decides its own display state and this software never changes it; " +
      "while the phone is locked the view is black and text is not delivered — unlock it on the phone.",
    iosMirroringTitle: "Use Apple's iPhone Mirroring on this Mac",
    iosMirroringDescription:
      "macOS ships iPhone Mirroring, which is the more complete experience; QuadControl just opens it for you " +
      "and everything else happens in that window. QuadControl never drives or automates that app.",
    iosMirroringOpen: "Open iPhone Mirroring",
    iosMirroringUnavailable: "Requires macOS 15 or newer",
    iosSessionElsewhere: "An iPhone session is already running",
    err_ios_link_lost: "The iPhone link was lost. Reconnect to continue.",
    err_no_active_element: "No text field is focused on the phone. Tap into a text field on the phone first.",
    err_unknown: "Device discovery is temporarily unavailable.",
    showMirror: "Show mirror window",
    disconnect: "Disconnect",
    sessionStatus: "Session status",
    sessionRunning: "Session running",
    sessionStarting: "Starting session…",
    sessionStopping: "Disconnecting…",
    sessionExited: "Session exited (code {code})",
    sessionFailed: "Session failed",
    futureVersion: "Coming in a future version",
    err_session_already_running: "This device already has a session in progress.",
    err_scrcpy_not_found: "scrcpy was not found. Install scrcpy 4.1 or newer and try again.",
    err_scrcpy_version: "This scrcpy version is unsupported. Install stable scrcpy 4.1 or newer.",
    err_session_start_failed: "The session could not start. Check the device connection and try again.",
    err_session_failed: "The session failed. Reconnect the device and try again.",
    mirrorDescription: "Your phone screen appears in a separate mirror window\n(scrcpy live view · click directly to control)",
    sessionControls: "Session controls",
    soundOutput: "Sound output",
    soundDescription: "Choose where phone audio plays",
    thisComputer: "This computer",
    phone: "Phone",
    screenOff: "Phone screen off",
    screenOffDescription: "Turn off the phone screen while keeping the view on this computer",
    on: "On",
    off: "Off",
    captureScreen: "Capture phone screen",
    captureDescription: "Save to the Pictures folder on this computer",
    capture: "Capture",
    safetyTitle: "You start every connection and can disconnect anytime",
    safetyDescription:
      "You authorized this computer on the phone yourself (developer options plus USB debugging). " +
      "You can disconnect at any time, and the phone's brightness, rotation, and timeout are restored " +
      "when the session ends; this software never bypasses its lock screen or password.",
    pairingDialog: "Add device pairing guide",
    chooseType: "Choose type",
    scanPair: "Scan to pair",
    startControl: "Start control",
    chooseDeviceTitle: "Choose a device to pair",
    chooseDeviceDescription: "Choose an Android phone to pair. iPhone support is coming in a later version.",
    androidPairing: "Pair with Android Wireless debugging",
    iphonePairing: "Not supported yet (coming later)",
    next: "Next",
    scanTitle: "Scan with Android Wireless debugging",
    qrPlaceholder: "Android Wireless debugging pairing QR code",
    qrCountdown: "Waiting for phone scan · {seconds} remaining",
    scanStepOne: "On the phone, open Settings → Developer options → Wireless debugging",
    scanStepTwo: "Tap “Pair device with QR code”",
    scanStepThree: "Point the phone at this QR code; the first setup may ask you to confirm trust in the current Wi‑Fi network",
    networkNote: "This is Android's native Wireless debugging pairing QR code; the phone's Settings app reads it.",
    manualLink: "Can't scan? Enter the pairing code manually",
    previous: "Back",
    waitingScan: "Waiting for phone…",
    manualDescription: "Enter the pairing address, pairing port, and six-digit code shown on the phone. If the device does not come online, enter the separate connection port shown on the Wireless debugging home screen.",
    hostLabel: "Pairing address shown on the phone",
    hostPlaceholder: "For example, 192.168.1.20",
    portLabel: "Pairing port shown on the phone",
    portPlaceholder: "1–65535",
    pairingCodeLabel: "Six-digit pairing code shown on the phone",
    pairingCodePlaceholder: "Enter six digits",
    continue: "Continue",
    pairingCompleteTitle: "Pairing complete — ready to control",
    pairingCompleteDescription: "Paired device: {device}",
    pairing_already_running: "A pairing flow is already in progress.",
    manual_pair_needs_connect_port: "Pairing succeeded, but the device is not online. Enter the separate connection port from the Wireless debugging home screen.",
    pair_failed: "Pairing failed. Check the phone state and try again.",
    pair_timed_out: "Pairing timed out. Reopen Wireless debugging and try again.",
    pair_cancelled: "Pairing cancelled.",
    done: "Done",
  },
};

const locale = navigator.language.toLowerCase().startsWith("zh") ? "zh" : "en";
const text = messages[locale];
document.documentElement.lang = locale === "zh" ? "zh-Hans" : "en";
document.title = text.appTitle;

document.querySelectorAll("[data-i18n]").forEach((element) => {
  element.textContent = text[element.dataset.i18n];
});
document.querySelectorAll("[data-i18n-aria-label]").forEach((element) => {
  element.setAttribute("aria-label", text[element.dataset.i18nAriaLabel]);
});
document.querySelectorAll("[data-i18n-placeholder]").forEach((element) => {
  element.setAttribute("placeholder", text[element.dataset.i18nPlaceholder]);
});
document.querySelectorAll("[data-i18n-title]").forEach((element) => {
  element.title = text[element.dataset.i18nTitle];
});

let devices = [];
let selectedDeviceId;
let discoveryErrors = [];
const sessionsByDevice = new Map();
const localSessionStates = new Map();
const preferencesByDevice = new Map();
const deviceList = document.querySelector("#device-list");
const deviceEmpty = document.querySelector("#device-empty");
const deviceErrors = document.querySelector("#device-errors");
const sessionHeader = document.querySelector("#session-header");
const sessionContent = document.querySelector("#session-content");
const sessionEmpty = document.querySelector("#session-empty");
const soundInputs = [...document.querySelectorAll('input[name="sound-output"]')];
const screenInputs = [...document.querySelectorAll('input[name="screen-off"]')];
const sessionActionControls = [
  document.querySelector("#show-mirror"),
  document.querySelector("#capture-screen"),
  ...soundInputs,
  ...screenInputs,
];

const sessionAction = document.querySelector("#session-action");
const showMirrorButton = document.querySelector("#show-mirror");

// 界面 C：iPhone 控制面板
const androidPanel = document.querySelector("#android-panel");
const iosPanel = document.querySelector("#ios-panel");
const iosStageColumn = document.querySelector("#ios-stage-column");
const iosStage = document.querySelector("#ios-stage");
const iosStream = document.querySelector("#ios-stream");
const iosStagePlaceholder = document.querySelector("#ios-stage-placeholder");
const iosFpsTag = document.querySelector("#ios-fps");
const iosLockedNote = document.querySelector("#ios-locked");
const iosTextCard = document.querySelector("#ios-text-card");
const iosTextInput = document.querySelector("#ios-text");
const iosSendButton = document.querySelector("#ios-send");
const iosTextResult = document.querySelector("#ios-text-result");
const iosActionCard = document.querySelector("#ios-action-card");
const iosActionResult = document.querySelector("#ios-action-result");
const iosWakeButton = document.querySelector("#ios-wake");
const iosCaptureButton = document.querySelector("#ios-capture");
const iosHomeButton = document.querySelector("#ios-home");
const iosMirroringCard = document.querySelector("#ios-mirroring-card");
const openMirroringButton = document.querySelector("#open-mirroring");
const mirroringNote = document.querySelector("#mirroring-note");

/// 宿主平台决定 iPhone 面板长什么样：macOS 只有引导卡，Win/Linux 才有画面与控制。
let hostPlatform = "linux";
let mirroringAvailable = false;
/// macOS 开发路径（debug 构建 + QUADCONTROL_IOS_DEV_WDA=1）：真机验收时用完整面板。
let iosDevWda = false;

/// macOS 的产品路径只有引导卡；开发路径放行时按 Win/Linux 版式渲染。
function iosGuidanceOnly() {
  return hostPlatform === "macos" && !iosDevWda;
}
/// 同一时刻只有一个 iOS 会话，所以状态是单个对象而不是 Map。
let iosStatus = { state: "idle", fps: 0 };
/// 会话属于哪台设备只在前端关联：状态 DTO 里**没有** UDID（红线），后端也不该有。
/// 代价是 WebView 重载会丢掉这个关联（会话仍在后台跑，状态卡照样能断开）。
let iosSessionDeviceId;
let iosStreaming = false;
let iosPollInFlight = false;

function iosSessionIsLive(state) {
  return state === "starting" || state === "running" || state === "stopping";
}

/// 后端说有会话在跑，但前端不知道它属于哪台设备（界面重载会丢掉这个关联，
/// 因为状态 DTO 里没有 UDID）。此时仍要让「断开连接」可达，但**不认领画面**：
/// 接了两台 iPhone 时把别人的屏幕显示在这台设备的面板上，比少显示一块画面糟糕。
function iosSessionIsUnowned() {
  return currentIosView().unowned;
}

function iosOwnsSelectedDevice() {
  const device = selectedDevice();
  return Boolean(device) && device.platform === "ios" && device.id === iosSessionDeviceId;
}

function preferencesFor(deviceId) {
  if (!preferencesByDevice.has(deviceId)) {
    preferencesByDevice.set(deviceId, { audioOnComputer: true, screenOff: false });
  }
  return preferencesByDevice.get(deviceId);
}

function selectedDevice() {
  return devices.find((device) => device.id === selectedDeviceId);
}

function renderDevices() {
  deviceList.replaceChildren();
  deviceEmpty.hidden = devices.length !== 0;
  deviceErrors.hidden = !discoveryErrors.length;
  deviceErrors.textContent = discoveryErrors.join(" ");
  devices.forEach((device) => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "device-item";
    button.dataset.deviceId = device.id;
    button.setAttribute("aria-current", String(device.id === selectedDeviceId));
    const statusText = deviceStatusText(device);
    button.setAttribute("aria-label", `${device.name}, ${text[device.platform]}, ${statusText}`);

    const phoneIcon = document.querySelector(".mirror-phone").cloneNode(true);
    phoneIcon.classList.remove("mirror-phone");
    phoneIcon.setAttribute("aria-hidden", "true");

    const copy = document.createElement("span");
    copy.className = "device-copy";
    const name = document.createElement("span");
    name.className = "device-name";
    name.textContent = device.name;
    const meta = document.createElement("span");
    meta.className = "device-meta";
    meta.textContent = `${text[device.platform]} · ${statusText}`;
    copy.append(name, meta);

    const dot = document.createElement("span");
    const tone = deviceIsControlled(device)
      ? "status-active"
      : device.status === "available"
        ? "status-available"
        : "status-pairing";
    dot.className = `status-dot ${tone}`;
    dot.setAttribute("aria-hidden", "true");
    button.append(phoneIcon, copy, dot);
    deviceList.append(button);
  });
}

// 正在被控制的设备在侧栏标「正在控制」，而不是设备发现给的原始状态。
function deviceIsControlled(device) {
  return (
    device.platform === "ios" &&
    device.id === iosSessionDeviceId &&
    iosSessionIsLive(iosStatus.state)
  );
}

function deviceStatusText(device) {
  return deviceIsControlled(device) ? text.controlling : text[device.status];
}

function renderSession() {
  const device = selectedDevice();
  const hasDevice = Boolean(device);
  sessionHeader.hidden = !hasDevice;
  sessionContent.hidden = !hasDevice;
  sessionEmpty.hidden = hasDevice;
  if (!device) {
    // 设备被拔掉时也要断掉 MJPEG 下游，别让隐藏的 <img> 继续拉流。
    syncIosStream();
    return;
  }
  document.querySelector("#session-device-name").textContent = device.name;
  if (device.platform === "ios") {
    renderIosPanel();
    return;
  }
  androidPanel.hidden = false;
  iosPanel.hidden = true;
  showMirrorButton.hidden = false;
  sessionAction.hidden = false;
  // 切到 Android 设备也要断掉 MJPEG 下游：隐藏的 <img> 仍然在拉流。
  syncIosStream();
  const session = sessionsByDevice.get(device.id);
  const state = localSessionStates.get(device.id) ?? session?.state ?? "idle";
  const preferences = preferencesFor(device.id);
  const stateText = state === "running" ? text.sessionRunning
    : state === "starting" ? text.sessionStarting
      : state === "stopping" ? text.sessionStopping
        : state === "exited" ? text.sessionExited.replace("{code}", String(session?.exit_code ?? "?"))
          : state === "failed" ? text.sessionFailed : text[device.status];
  document.querySelector("#session-state").textContent = stateText;
  document.querySelector("#session-connection").textContent = text[device.platform];
  sessionAction.textContent = state === "running" || state === "stopping" ? text.disconnect : text.startControl;
  sessionAction.classList.toggle("btn-primary", state !== "running" && state !== "stopping");
  sessionAction.classList.toggle("btn-disconnect", state === "running" || state === "stopping");
  sessionAction.disabled = state === "starting" || state === "stopping";
  const controlsLocked = state === "running" || state === "starting" || state === "stopping";
  sessionActionControls.forEach((control) => { control.disabled = controlsLocked || control.id === "show-mirror" || control.id === "capture-screen"; });
  soundInputs.forEach((input) => { input.checked = input.value === (preferences.audioOnComputer ? "computer" : "phone"); });
  screenInputs.forEach((input) => { input.checked = input.value === (preferences.screenOff ? "on" : "off"); });
  const detail = document.querySelector("#session-status-detail");
  detail.textContent = state === "failed" ? describeError(session?.stderr_tail ?? "session_failed")
    : state === "exited" ? stateText : stateText;
  const stderr = document.querySelector("#session-stderr");
  // 失败时 stderr_tail 是稳定错误码，翻译后显示；非零退出时是 scrcpy 的原始
  // 尾部输出（诊断用，仅此一处允许原文）。
  const showTail = Boolean(session?.stderr_tail) && (state === "failed" || (state === "exited" && session?.exit_code !== 0));
  stderr.hidden = !showTail;
  stderr.textContent = !showTail ? "" : state === "failed" ? describeError(session.stderr_tail) : session.stderr_tail;
}

// 后端只回稳定错误码，这里翻译；认不出的码退回通用文案，绝不把原始英文抛给用户。
function describeError(code) {
  return text[`err_${code}`] ?? text.err_unknown;
}

function describePairingError(code) {
  return text[code] ?? describeError(code);
}

let refreshInFlight = false;

async function refreshDevices() {
  // adb 卡住时一次调用可能远超 5 秒，不加这道闸会越堆越多。
  if (refreshInFlight) return;
  refreshInFlight = true;
  try {
    const [result, sessionResult] = await Promise.all([
      window.__TAURI__.core.invoke("list_devices"),
      window.__TAURI__.core.invoke("sessions"),
    ]);
    devices = result.devices ?? [];
    sessionsByDevice.clear();
    (sessionResult ?? []).forEach((session) => sessionsByDevice.set(session.device_id, session));
    // 本地过渡态（starting/stopping）只是为了点击后立即反馈；后端一旦给出终态
    // 就以后端为准，否则断开成功后按钮会永远停在「正在断开连接…」。
    localSessionStates.forEach((_, deviceId) => {
      const backend = sessionsByDevice.get(deviceId)?.state;
      if (backend !== "running" && backend !== "stopping") localSessionStates.delete(deviceId);
    });
    discoveryErrors = [result.android_error, result.ios_error]
      .filter(Boolean)
      .map(describeError);
    if (!devices.some((device) => device.id === selectedDeviceId)) {
      selectedDeviceId = devices[0]?.id;
    }
  } catch (error) {
    console.error("list_devices failed", error);
    discoveryErrors = [text.err_unknown];
  } finally {
    refreshInFlight = false;
  }
  renderDevices();
  renderSession();
}

deviceList.addEventListener("click", (event) => {
  const button = event.target.closest(".device-item");
  if (!button) return;
  selectedDeviceId = button.dataset.deviceId;
  renderDevices();
  renderSession();
});

sessionAction.addEventListener("click", async () => {
  const device = selectedDevice();
  if (!device) return;
  if (device.platform === "ios") {
    await handleIosSessionAction(device);
    return;
  }
  const state = localSessionStates.get(device.id) ?? sessionsByDevice.get(device.id)?.state;
  if (state === "running") {
    localSessionStates.set(device.id, "stopping");
    renderSession();
    try {
      await window.__TAURI__.core.invoke("stop_session", { deviceId: device.id });
      await waitForSessionSettled(device.id);
    } catch (error) {
      console.error("stop_session failed", error);
    }
    localSessionStates.delete(device.id);
    await refreshDevices();
    return;
  }
  const preferences = preferencesFor(device.id);
  localSessionStates.set(device.id, "starting");
  renderSession();
  try {
    await window.__TAURI__.core.invoke("start_session", {
      deviceId: device.id,
      screenOff: preferences.screenOff,
      audioOnComputer: preferences.audioOnComputer,
    });
    localSessionStates.delete(device.id);
    await refreshDevices();
  } catch (error) {
    console.error("start_session failed", error);
    localSessionStates.delete(device.id);
    sessionsByDevice.set(device.id, { device_id: device.id, state: "failed", stderr_tail: String(error) });
    renderSession();
  }
});

// 断开后端最多要走 7 秒的终止梯子；在此期间保持「正在断开连接…」，
// 直到后端不再报 running/stopping 或超出预算。
async function waitForSessionSettled(deviceId) {
  const deadline = Date.now() + 8000;
  while (Date.now() < deadline) {
    const sessions = await window.__TAURI__.core.invoke("sessions");
    const state = (sessions ?? []).find((session) => session.device_id === deviceId)?.state;
    if (state !== "running" && state !== "stopping") return;
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
}

soundInputs.forEach((input) => input.addEventListener("change", () => {
  const device = selectedDevice();
  if (!device) return;
  preferencesFor(device.id).audioOnComputer = input.value === "computer";
}));
screenInputs.forEach((input) => input.addEventListener("change", () => {
  const device = selectedDevice();
  if (!device) return;
  preferencesFor(device.id).screenOff = input.value === "on";
}));

// ---------------------------------------------------------------------------
// 界面 C：iPhone 控制面板
// ---------------------------------------------------------------------------

/// 状态 + 归属 + 平台 → 渲染决策。**纯函数**：只读参数与 `text`，不碰 DOM，
/// 也不读模块里的可变状态。界面 C 的所有分支判断都集中在这里，applyIosStatus
/// 只负责把结果写进 DOM。
function iosViewModel({ status, ownsSelected, guidanceOnly, platform }) {
  const state = ownsSelected ? (status.state ?? "idle") : "idle";
  // 后端说有会话在跑，但前端不知道它属于谁（界面重载会丢掉归属）。
  const unowned = !ownsSelected && iosSessionIsLive(status.state);
  const running = state === "running";
  const locked = running && status.locked === true;
  // Windows 上收不回进程树，与 Android 面板一样明确拒绝，而不是让用户点了才知道。
  const windowsBlocked = platform === "windows";

  let statusText;
  if (unowned) {
    statusText = text.iosSessionElsewhere;
  } else if (state === "failed") {
    statusText = describeError(status.code ?? "session_failed");
  } else if (state === "exited" && status.exit_reason && status.exit_reason !== "requested") {
    // 自己退的会话不报错；被链路拖死的要说明原因。
    statusText = describeError(
      status.exit_reason === "upstream_closed" ? "proxy_failed" : "ios_link_lost",
    );
  } else if (state === "starting") {
    statusText = text.sessionStarting;
  } else if (state === "stopping") {
    statusText = text.sessionStopping;
  } else if (running) {
    statusText = text.sessionRunning;
  } else if (windowsBlocked) {
    statusText = describeError("windows_session_unsupported");
  } else {
    statusText = text.iosDisconnected;
  }

  const isDisconnect = running || state === "stopping" || unowned;
  return {
    state,
    unowned,
    running,
    locked,
    statusText,
    stageText: running
      ? text.iosConnected.replace("{fps}", String(status.fps ?? 0))
      : statusText,
    actionLabel: isDisconnect ? text.disconnect : text.startControl,
    actionIsDisconnect: isDisconnect,
    actionDisabled: state === "starting" || state === "stopping" || (windowsBlocked && !isDisconnect),
    textEnabled: running && !locked,
    actionsEnabled: running,
    aspectRatio: running && status.window ? `${status.window.width} / ${status.window.height}` : "",
    shouldStream: !guidanceOnly && ownsSelected && running && Boolean(status.proxy_port),
    proxyPort: status.proxy_port,
  };
}

/// 当前选中设备对应的视图模型。
function currentIosView() {
  return iosViewModel({
    status: iosStatus,
    ownsSelected: iosOwnsSelectedDevice(),
    guidanceOnly: iosGuidanceOnly(),
    platform: hostPlatform,
  });
}

/// 只做「平台决定的骨架」：显示哪些卡片、按钮在不在。每秒轮询走 applyIosStatus。
function renderIosPanel() {
  androidPanel.hidden = true;
  iosPanel.hidden = false;
  const isMac = iosGuidanceOnly();
  document.querySelector("#session-connection").textContent = text.ios;
  showMirrorButton.hidden = true;
  // macOS 上根本不给「开始控制」：那条路的产品答案是系统「iPhone 镜像」。
  // 后端的 `macos_uses_iphone_mirroring` 是第二道闸，不是这里的提示来源。
  sessionAction.hidden = isMac;
  iosStageColumn.hidden = isMac;
  iosTextCard.hidden = isMac;
  iosActionCard.hidden = isMac;
  iosMirroringCard.hidden = !isMac;
  if (isMac) {
    openMirroringButton.disabled = !mirroringAvailable;
    mirroringNote.hidden = mirroringAvailable;
    mirroringNote.textContent = mirroringAvailable ? "" : text.iosMirroringUnavailable;
  }
  applyIosStatus();
}

/// 每秒轮询只改文案与 disabled，绝不重建 DOM、绝不碰 `img.src`。
function applyIosStatus() {
  const device = selectedDevice();
  if (!device || device.platform !== "ios") {
    // 选中的不是 iPhone：面板不归它管，但流该断还是要断。
    syncIosStream();
    return;
  }
  const view = currentIosView();

  document.querySelector("#session-state").textContent = view.statusText;
  sessionAction.textContent = view.actionLabel;
  sessionAction.classList.toggle("btn-primary", !view.actionIsDisconnect);
  sessionAction.classList.toggle("btn-disconnect", view.actionIsDisconnect);
  sessionAction.disabled = view.actionDisabled;

  iosFpsTag.textContent = view.stageText;
  iosLockedNote.hidden = !view.locked;
  // 画面容器按设备长宽比定尺；后端用同一个 window 算内容矩形，两边的 letterbox
  // 才是同一个（见 `content_point`）。
  iosStage.style.aspectRatio = view.aspectRatio;
  iosStage.classList.toggle("is-live", view.running);

  // 锁定时不转发文字：WDA 在锁屏下返回成功但字符不送达（真机测量）。
  iosTextInput.disabled = !view.textEnabled;
  iosSendButton.disabled = !view.textEnabled;
  [iosWakeButton, iosCaptureButton, iosHomeButton].forEach((button) => {
    button.disabled = !view.actionsEnabled;
  });

  syncIosStream();
  iosStagePlaceholder.hidden = iosStreaming;
}

/// MJPEG 是无限流：`src` **只在状态迁移时**设置或清空。每秒重设会让画面每秒重连。
function syncIosStream() {
  const device = selectedDevice();
  const shouldStream = device?.platform === "ios" && currentIosView().shouldStream;
  if (shouldStream === iosStreaming) return;
  iosStreaming = shouldStream;
  if (shouldStream) {
    iosStream.src = `http://127.0.0.1:${iosStatus.proxy_port}/`;
  } else {
    iosStream.removeAttribute("src");
  }
}

async function pollIosStatus() {
  const device = selectedDevice();
  // 选中 Android 设备时也要继续轮询**自己拥有的** iOS 会话：否则会话死了，侧栏
  // 还在标「正在控制」，切回来的那一秒还会拿陈旧的 proxy_port 去设 img.src。
  if (iosGuidanceOnly() || !(device?.platform === "ios" || iosSessionDeviceId)) return;
  // 一次轮询里有两次 5 秒超时的 WDA 查询，不加闸会越堆越多。
  if (iosPollInFlight) return;
  iosPollInFlight = true;
  const wasLive = iosSessionIsLive(iosStatus.state);
  try {
    const next = await window.__TAURI__.core.invoke("ios_session_status");
    // 平台闸门（`macos_uses_iphone_mirroring` / `windows_session_unsupported`）
    // 在后端建槽位之前就返回了，所以那类失败只存在于前端。后端回 idle 时不要
    // 覆盖它，否则错误码闪一下就没了。
    const keepLocalFailure =
      next.state === "idle" && iosStatus.state === "failed" && Boolean(iosSessionDeviceId);
    if (!keepLocalFailure) {
      iosStatus = next;
      if (next.state === "idle") iosSessionDeviceId = undefined;
    }
  } catch (error) {
    console.error("ios_session_status failed", error);
  } finally {
    iosPollInFlight = false;
  }
  applyIosStatus();
  // 侧栏的「正在控制」只在会话活/死翻转时重建，避免每秒重画设备列表。
  if (iosSessionIsLive(iosStatus.state) !== wasLive) renderDevices();
}

// 断开后端最多走 7 秒的终止梯子；期间保持「正在断开连接…」。
async function waitForIosSettled() {
  const deadline = Date.now() + 8000;
  while (Date.now() < deadline) {
    try {
      const status = await window.__TAURI__.core.invoke("ios_session_status");
      if (!iosSessionIsLive(status.state)) return;
    } catch (error) {
      console.error("ios_session_status failed", error);
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
}

async function handleIosSessionAction(device) {
  const state = iosOwnsSelectedDevice() ? iosStatus.state : "idle";
  if (iosSessionIsLive(state) || iosSessionIsUnowned()) {
    iosStatus = { ...iosStatus, state: "stopping" };
    applyIosStatus();
    try {
      await window.__TAURI__.core.invoke("stop_ios_session");
      await waitForIosSettled();
    } catch (error) {
      console.error("stop_ios_session failed", error);
    }
    iosSessionDeviceId = undefined;
    iosStatus = { state: "idle", fps: 0 };
    applyIosStatus();
    renderDevices();
    await refreshDevices();
    return;
  }
  iosSessionDeviceId = device.id;
  iosStatus = { state: "starting", fps: 0 };
  applyIosStatus();
  try {
    await window.__TAURI__.core.invoke("start_ios_session", { deviceId: device.id });
    await pollIosStatus();
  } catch (error) {
    console.error("start_ios_session failed", error);
    // **保留归属**：失败属于这台设备。清掉的话 applyIosStatus 会把它当 idle，
    // 整张启动错误码表就永远显示不出来。下一次 start 或后端报 idle 之外的状态
    // 会覆盖它（轮询里的 keepLocalFailure 负责不让 idle 抹掉它）。
    iosStatus = { state: "failed", code: String(error), fps: 0 };
    applyIosStatus();
  }
  renderDevices();
}

function showIosNote(target, message) {
  target.textContent = message;
  target.hidden = false;
}

async function invokeIosAction(command, args, onSuccess) {
  try {
    const result = await window.__TAURI__.core.invoke(command, args);
    onSuccess(result);
  } catch (error) {
    console.error(`${command} failed`, error);
    showIosNote(iosActionResult, describeError(String(error)));
  }
}

iosStage.addEventListener("click", async (event) => {
  if (!iosOwnsSelectedDevice() || iosStatus.state !== "running") return;
  // 锁定时点画面 = 唤醒屏幕，不转发点按（锁屏下点按不产生效果）。
  if (iosStatus.locked === true) {
    await invokeIosAction("ios_wake", undefined, () => {
      iosActionResult.hidden = true;
    });
    return;
  }
  const rect = iosStage.getBoundingClientRect();
  // 传容器内像素坐标 + 容器尺寸，由后端按设备长宽比算内容矩形并归一化。
  await invokeIosAction(
    "ios_tap",
    {
      xPx: event.clientX - rect.left,
      yPx: event.clientY - rect.top,
      widthPx: rect.width,
      heightPx: rect.height,
    },
    () => {
      iosActionResult.hidden = true;
    },
  );
});

async function sendIosText() {
  const value = iosTextInput.value;
  if (!value.trim()) return;
  iosSendButton.disabled = true;
  try {
    await window.__TAURI__.core.invoke("ios_send_text", { text: value });
    showIosNote(iosTextResult, text.iosTextSent);
    iosTextInput.value = "";
  } catch (error) {
    console.error("ios_send_text failed", error);
    showIosNote(iosTextResult, describeError(String(error)));
  }
  // 交回视图模型决定按钮状态：直接置 false 会盖掉锁定/已断开时的禁用。
  applyIosStatus();
}

iosSendButton.addEventListener("click", sendIosText);
iosTextInput.addEventListener("keydown", (event) => {
  if (event.key === "Enter") {
    event.preventDefault();
    sendIosText();
  }
});

iosWakeButton.addEventListener("click", () =>
  invokeIosAction("ios_wake", undefined, () => {
    iosActionResult.hidden = true;
  }),
);
iosHomeButton.addEventListener("click", () =>
  invokeIosAction("ios_home", undefined, () => {
    iosActionResult.hidden = true;
  }),
);
iosCaptureButton.addEventListener("click", () =>
  invokeIosAction("ios_screenshot", undefined, (path) => {
    showIosNote(iosActionResult, text.iosScreenshotSaved.replace("{path}", path));
  }),
);

openMirroringButton.addEventListener("click", async () => {
  try {
    await window.__TAURI__.core.invoke("open_iphone_mirroring");
    mirroringNote.hidden = true;
  } catch (error) {
    console.error("open_iphone_mirroring failed", error);
    showIosNote(mirroringNote, describeError(String(error)));
  }
});

async function loadHostPlatform() {
  try {
    hostPlatform = await window.__TAURI__.core.invoke("host_platform");
    mirroringAvailable = await window.__TAURI__.core.invoke("iphone_mirroring_available");
    iosDevWda = await window.__TAURI__.core.invoke("ios_dev_wda_enabled");
  } catch (error) {
    console.error("host_platform failed", error);
  }
  renderSession();
}

renderDevices();
renderSession();
loadHostPlatform();
refreshDevices();
window.setInterval(refreshDevices, 5000);
window.setInterval(pollIosStatus, 1000);

const backdrop = document.querySelector("#pairing-backdrop");
const dialog = backdrop.querySelector(".dialog");
const stepViews = [1, 2, 3].map((step) => document.querySelector(`#pair-step-${step}`));
const scanView = document.querySelector("#scan-view");
const manualForm = document.querySelector("#manual-form");
let pairingStep = 1;
let countdownTimer;
let pairingGeneration;
let pairingDeviceName = "Android";

function renderPairingSteps() {
  const labels = [text.chooseType, text.scanPair, text.startControl];
  const stepper = document.querySelector("#pairing-steps");
  stepper.setAttribute("aria-label", text.pairingDialog);
  stepper.replaceChildren();
  labels.forEach((label, index) => {
    const stepNumber = index + 1;
    const item = document.createElement("span");
    item.className = "step-item";
    if (stepNumber < pairingStep) item.classList.add("is-complete");
    if (stepNumber === pairingStep) item.classList.add("is-current");

    const marker = document.createElement("span");
    marker.className = "step-marker";
    if (stepNumber < pairingStep) {
      marker.innerHTML = '<svg class="icon" aria-hidden="true"><use href="#icon-check"></use></svg>';
    } else {
      marker.textContent = String(stepNumber);
    }
    const name = document.createElement("span");
    name.textContent = label;
    item.append(marker, name);
    stepper.append(item);
    if (stepNumber < labels.length) {
      const line = document.createElement("span");
      line.className = "step-line";
      line.setAttribute("aria-hidden", "true");
      stepper.append(line);
    }
  });
}

// 扫码与手动两条路都靠这个轮询拿结果：后端命令只表示 worker 已起飞。
function pairingStatusTarget() {
  return manualForm.hidden ? document.querySelector("#qr-countdown") : document.querySelector("#manual-error");
}

function updateCountdown() {
  return window.__TAURI__.core.invoke("pairing_status").then((status) => {
    if (status.generation !== pairingGeneration) return;
    const target = pairingStatusTarget();
    if (!target) return;
    target.hidden = false;
    target.textContent = manualForm.hidden
      ? text.qrCountdown.replace("{seconds}", formatRemaining(status.remaining_secs ?? 0))
      : text.waitingScan;
    if (status.state === "succeeded") {
      pairingDeviceName = status.device_name ?? "Android";
      document.querySelector("[data-i18n='pairingCompleteDescription']").textContent = text.pairingCompleteDescription.replace("{device}", pairingDeviceName);
      clearInterval(countdownTimer);
      clearQrCode();
      setPairingStep(3);
      refreshDevices();
    } else if (status.state === "failed") {
      clearInterval(countdownTimer);
      target.textContent = describePairingError(status.error_code ?? "pair_failed");
    }
  }).catch(() => {});
}

function formatRemaining(seconds) {
  const minutes = Math.floor(seconds / 60);
  return `${minutes}:${String(seconds % 60).padStart(2, "0")}`;
}

function startPairingStatusPoll() {
  clearInterval(countdownTimer);
  updateCountdown();
  countdownTimer = window.setInterval(updateCountdown, 1000);
}

function showScanView() {
  manualForm.hidden = true;
  document.querySelector("#manual-error").hidden = true;
  scanView.hidden = false;
  startPairingStatusPoll();
  document.querySelector("#show-manual").focus();
}

function setPairingStep(step) {
  pairingStep = step;
  stepViews.forEach((view, index) => {
    view.hidden = index + 1 !== step;
  });
  renderPairingSteps();
  if (step === 2) {
    showScanView();
  } else {
    clearInterval(countdownTimer);
    const target = step === 1 ? document.querySelector("#pair-next") : document.querySelector("#pair-done");
    target.focus();
  }
}

function openPairing() {
  backdrop.hidden = false;
  document.body.classList.add("dialog-open");
  setPairingStep(1);
}

async function cancelPairing() {
  try { await window.__TAURI__.core.invoke("cancel_pairing"); } catch (error) { console.error("cancel_pairing failed", error); }
}

/// 二维码矩阵是配对密码的等价编码，轮次一结束就要从 DOM 里清掉，
/// 不能只把对话框隐藏了事——隐藏的节点仍然留在文档里。
function clearQrCode() {
  document.querySelector("#qr-code")?.replaceChildren();
}

async function closePairing() {
  clearInterval(countdownTimer);
  await cancelPairing();
  clearQrCode();
  backdrop.hidden = true;
  document.body.classList.remove("dialog-open");
  document.querySelector("#add-device").focus();
}

document.querySelector("#add-device").addEventListener("click", openPairing);
document.querySelector("#pair-next").addEventListener("click", async () => {
  try {
    const start = await window.__TAURI__.core.invoke("start_pairing");
    pairingGeneration = start.generation;
    const qr = document.querySelector("#qr-code");
    qr.replaceChildren();
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute("viewBox", `0 0 ${start.side} ${start.side}`);
    svg.setAttribute("aria-hidden", "true");
    const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
    let d = "";
    for (let y = 0; y < start.side; y += 1) for (let x = 0; x < start.side; x += 1) if (start.modules[y * start.side + x]) d += `M${x} ${y}h1v1H${x}z`;
    path.setAttribute("d", d);
    path.setAttribute("fill", "#111111");
    svg.append(path);
    const countdown = document.createElement("span");
    countdown.id = "qr-countdown";
    qr.append(svg, countdown);
    setPairingStep(2);
  } catch (error) {
    document.querySelector("#qr-countdown").textContent = describePairingError(String(error));
  }
});
document.querySelector("#show-manual").addEventListener("click", async () => {
  clearInterval(countdownTimer);
  // 切到手动模式必须真正取消二维码轮次：否则两路 adb pair 会并发，
  // 后端也会因为已有在飞配对而拒绝手动提交。
  await cancelPairing();
  clearQrCode();
  scanView.hidden = true;
  manualForm.hidden = false;
  manualForm.elements.host.focus();
});
document.querySelector("#manual-back").addEventListener("click", showScanView);
manualForm.addEventListener("submit", (event) => {
  event.preventDefault();
  if (!manualForm.reportValidity()) return;
  const port = Number(manualForm.elements.port.value);
  const code = manualForm.elements.code.value;
  if (!Number.isInteger(port) || port < 1 || port > 65535 || !/^\d{6}$/.test(code)) return;
  window.__TAURI__.core.invoke("pair_manual", { host: manualForm.elements.host.value, port, code })
    .then((generation) => {
      // 命令返回只代表 worker 已起飞；成功/失败由轮询按 generation 取回。
      pairingGeneration = generation;
      startPairingStatusPoll();
    })
    .catch((error) => {
      const message = describePairingError(String(error));
      const errorView = document.querySelector("#manual-error");
      errorView.textContent = message;
      errorView.hidden = false;
    });
  manualForm.elements.host.value = "";
  manualForm.elements.port.value = "";
  manualForm.elements.code.value = "";
});
document.querySelectorAll(".pair-back").forEach((button) => {
  button.addEventListener("click", async () => {
    await cancelPairing();
    clearQrCode();
    setPairingStep(pairingStep - 1);
  });
});
document.querySelector("#pair-done").addEventListener("click", closePairing);

backdrop.addEventListener("click", closePairing);
dialog.addEventListener("click", (event) => event.stopPropagation());
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && !backdrop.hidden) closePairing();
});
