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
    button.setAttribute(
      "aria-label",
      `${device.name}, ${text[device.platform]}, ${text[device.status]}`,
    );

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
    meta.textContent = `${text[device.platform]} · ${text[device.status]}`;
    copy.append(name, meta);

    const dot = document.createElement("span");
    dot.className = `status-dot ${device.status === "available" ? "status-available" : "status-pairing"}`;
    dot.setAttribute("aria-hidden", "true");
    button.append(phoneIcon, copy, dot);
    deviceList.append(button);
  });
}

function renderSession() {
  const device = selectedDevice();
  const hasDevice = Boolean(device);
  sessionHeader.hidden = !hasDevice;
  sessionContent.hidden = !hasDevice;
  sessionEmpty.hidden = hasDevice;
  if (!device) return;
  document.querySelector("#session-device-name").textContent = device.name;
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

renderDevices();
renderSession();
refreshDevices();
window.setInterval(refreshDevices, 5000);

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
