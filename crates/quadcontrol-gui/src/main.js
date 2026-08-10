const messages = {
  zh: {
    appTitle: "QuadControl",
    myDevices: "我的设备",
    addDevice: "添加设备",
    android: "安卓",
    androidName: "Android",
    iphone: "iPhone",
    pixelName: "Pixel 8",
    galaxyName: "Galaxy S24",
    iphoneName: "iPhone SE",
    controlling: "正在控制",
    available: "可连接",
    needsPairing: "需要配对",
    sessionActive: "会话进行中",
    sessionAvailable: "设备可连接",
    wifiLatency: "Wi-Fi · 延迟 38 ms",
    wifiReady: "Wi-Fi · 尚未连接",
    pairingRequired: "需要先完成配对",
    showMirror: "显示镜像窗口",
    disconnect: "断开连接",
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
    iphone: "iPhone",
    pixelName: "Pixel 8",
    galaxyName: "Galaxy S24",
    iphoneName: "iPhone SE",
    controlling: "Controlling",
    available: "Ready to connect",
    needsPairing: "Pairing required",
    sessionActive: "Session active",
    sessionAvailable: "Ready to connect",
    wifiLatency: "Wi-Fi · 38 ms latency",
    wifiReady: "Wi-Fi · Not connected",
    pairingRequired: "Pairing required first",
    showMirror: "Show mirror window",
    disconnect: "Disconnect",
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

const devices = [
  {
    id: "pixel-8",
    nameKey: "pixelName",
    platformKey: "android",
    statusKey: "controlling",
    stateKey: "sessionActive",
    connectionKey: "wifiLatency",
    dotClass: "status-active",
    active: true,
    soundOutput: "computer",
    screenOff: "off",
  },
  {
    id: "galaxy-s24",
    nameKey: "galaxyName",
    platformKey: "android",
    statusKey: "available",
    stateKey: "sessionAvailable",
    connectionKey: "wifiReady",
    dotClass: "status-available",
    active: false,
    soundOutput: "computer",
    screenOff: "off",
  },
  {
    id: "iphone-se",
    nameKey: "iphoneName",
    platformKey: "iphone",
    statusKey: "needsPairing",
    stateKey: "needsPairing",
    connectionKey: "pairingRequired",
    dotClass: "status-pairing",
    active: false,
    soundOutput: "computer",
    screenOff: "off",
  },
];

let selectedDeviceId = devices[0].id;
const deviceList = document.querySelector("#device-list");
const soundInputs = [...document.querySelectorAll('input[name="sound-output"]')];
const screenInputs = [...document.querySelectorAll('input[name="screen-off"]')];
const sessionActionControls = [
  document.querySelector("#show-mirror"),
  document.querySelector("#disconnect"),
  document.querySelector(".control-row .btn-secondary"),
  ...soundInputs,
  ...screenInputs,
];

function selectedDevice() {
  return devices.find((device) => device.id === selectedDeviceId);
}

function renderDevices() {
  deviceList.replaceChildren();
  devices.forEach((device) => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "device-item";
    button.dataset.deviceId = device.id;
    button.setAttribute("aria-current", String(device.id === selectedDeviceId));
    button.setAttribute(
      "aria-label",
      `${text[device.nameKey]}, ${text[device.platformKey]}, ${text[device.statusKey]}`,
    );

    const phoneIcon = document.querySelector(".mirror-phone").cloneNode(true);
    phoneIcon.classList.remove("mirror-phone");
    phoneIcon.setAttribute("aria-hidden", "true");

    const copy = document.createElement("span");
    copy.className = "device-copy";
    const name = document.createElement("span");
    name.className = "device-name";
    name.textContent = text[device.nameKey];
    const meta = document.createElement("span");
    meta.className = "device-meta";
    meta.textContent = `${text[device.platformKey]} · ${text[device.statusKey]}`;
    copy.append(name, meta);

    const dot = document.createElement("span");
    dot.className = `status-dot ${device.dotClass}`;
    dot.setAttribute("aria-hidden", "true");
    button.append(phoneIcon, copy, dot);
    deviceList.append(button);
  });
}

function renderSession() {
  const device = selectedDevice();
  document.querySelector("#session-device-name").textContent = text[device.nameKey];
  document.querySelector("#session-state").textContent = text[device.stateKey];
  document.querySelector("#session-connection").textContent = text[device.connectionKey];
  sessionActionControls.forEach((control) => {
    control.disabled = !device.active;
  });
  soundInputs.find((input) => input.value === device.soundOutput).checked = true;
  screenInputs.find((input) => input.value === device.screenOff).checked = true;
}

deviceList.addEventListener("click", (event) => {
  const button = event.target.closest(".device-item");
  if (!button) return;
  selectedDeviceId = button.dataset.deviceId;
  renderDevices();
  renderSession();
});

soundInputs.forEach((input) => {
  input.addEventListener("change", () => {
    selectedDevice().soundOutput = input.value;
  });
});
screenInputs.forEach((input) => {
  input.addEventListener("change", () => {
    selectedDevice().screenOff = input.value;
  });
});

renderDevices();
renderSession();

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
