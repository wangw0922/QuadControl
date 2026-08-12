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
    safetyDescription: "控制期间手机上会一直显示提示；本软件不会绕过锁屏或密码。",
    pairingDialog: "添加设备配对向导",
    chooseType: "选择类型",
    scanPair: "扫码配对",
    startControl: "开始控制",
    chooseDeviceTitle: "选择要配对的设备",
    chooseDeviceDescription: "选择手机类型，然后在下一步用手机扫描配对码。",
    androidPairing: "使用 QuadControl 扫码配对",
    iphonePairing: "使用 QuadControl 扫码配对",
    next: "下一步",
    scanTitle: "用手机扫一下，就连好了",
    qrPlaceholder: "配对二维码占位",
    qrCountdown: "配对二维码 · {seconds} 秒自动刷新",
    scanStepOne: "在手机上打开 QuadControl",
    scanStepTwo: "点击「扫描配对码」",
    scanStepThree: "把相机对准左边的二维码，地址和密钥会自动填好",
    networkNote: "二维码只在你的家庭/办公网络内有效，不会经过互联网。",
    manualLink: "无法扫码？手动输入配对码",
    previous: "上一步",
    waitingScan: "等待手机扫码…",
    manualDescription: "输入电脑显示的地址、端口和配对密钥。",
    hostLabel: "电脑地址",
    hostPlaceholder: "例如 192.168.1.20",
    portLabel: "端口",
    portPlaceholder: "例如 27183",
    pairingCodeLabel: "配对密钥",
    pairingCodePlaceholder: "输入配对密钥",
    continue: "继续",
    pairingCompleteTitle: "配对完成，可以开始控制",
    pairingCompleteDescription: "这台手机已经添加到你的设备列表。",
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
    safetyDescription: "A notice remains visible on the phone during control; this software never bypasses its lock screen or password.",
    pairingDialog: "Add device pairing guide",
    chooseType: "Choose type",
    scanPair: "Scan to pair",
    startControl: "Start control",
    chooseDeviceTitle: "Choose a device to pair",
    chooseDeviceDescription: "Choose the phone type, then scan the pairing code with your phone.",
    androidPairing: "Scan with QuadControl to pair",
    iphonePairing: "Scan with QuadControl to pair",
    next: "Next",
    scanTitle: "Scan once and you're connected",
    qrPlaceholder: "Pairing QR code placeholder",
    qrCountdown: "Pairing QR · refreshes in {seconds} seconds",
    scanStepOne: "Open QuadControl on your phone",
    scanStepTwo: "Tap “Scan pairing code”",
    scanStepThree: "Point the camera at the QR code; the address and key fill in automatically",
    networkNote: "This QR code works only on your home or office network and never passes through the internet.",
    manualLink: "Can't scan? Enter the pairing code manually",
    previous: "Back",
    waitingScan: "Waiting for phone…",
    manualDescription: "Enter the address, port, and pairing key shown on your computer.",
    hostLabel: "Computer address",
    hostPlaceholder: "For example, 192.168.1.20",
    portLabel: "Port",
    portPlaceholder: "For example, 27183",
    pairingCodeLabel: "Pairing key",
    pairingCodePlaceholder: "Enter pairing key",
    continue: "Continue",
    pairingCompleteTitle: "Pairing complete — ready to control",
    pairingCompleteDescription: "This phone is now in your device list.",
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
  stderr.hidden = state !== "failed" || !session?.stderr_tail;
  stderr.textContent = stderr.hidden ? "" : describeError(session.stderr_tail);
}

// 后端只回稳定错误码，这里翻译；认不出的码退回通用文案，绝不把原始英文抛给用户。
function describeError(code) {
  return text[`err_${code}`] ?? text.err_unknown;
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
    } catch (error) {
      console.error("stop_session failed", error);
      localSessionStates.delete(device.id);
      renderSession();
    }
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
  } catch (error) {
    console.error("start_session failed", error);
    localSessionStates.delete(device.id);
    sessionsByDevice.set(device.id, { device_id: device.id, state: "failed", stderr_tail: String(error) });
  }
  renderSession();
});

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
let countdown = 30;
let countdownTimer;

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

function updateCountdown() {
  document.querySelector("#qr-countdown").textContent = text.qrCountdown.replace(
    "{seconds}",
    String(countdown),
  );
}

function startCountdown() {
  clearInterval(countdownTimer);
  countdown = 30;
  updateCountdown();
  countdownTimer = window.setInterval(() => {
    countdown = countdown > 1 ? countdown - 1 : 30;
    updateCountdown();
  }, 1000);
}

function showScanView() {
  manualForm.hidden = true;
  scanView.hidden = false;
  startCountdown();
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

function closePairing() {
  clearInterval(countdownTimer);
  backdrop.hidden = true;
  document.body.classList.remove("dialog-open");
  document.querySelector("#add-device").focus();
}

document.querySelector("#add-device").addEventListener("click", openPairing);
document.querySelector("#pair-next").addEventListener("click", () => setPairingStep(2));
document.querySelector("#show-manual").addEventListener("click", () => {
  clearInterval(countdownTimer);
  scanView.hidden = true;
  manualForm.hidden = false;
  manualForm.elements.host.focus();
});
document.querySelector("#manual-back").addEventListener("click", showScanView);
manualForm.addEventListener("submit", (event) => {
  event.preventDefault();
  if (manualForm.reportValidity()) setPairingStep(3);
});
document.querySelectorAll(".pair-back").forEach((button) => {
  button.addEventListener("click", () => setPairingStep(pairingStep - 1));
});
document.querySelector("#pair-done").addEventListener("click", closePairing);

backdrop.addEventListener("click", closePairing);
dialog.addEventListener("click", (event) => event.stopPropagation());
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && !backdrop.hidden) closePairing();
});
