const messages = {
  zh: {
    title: "QuadControl 控制台",
    subtitle: "跨平台桌面控制端空壳",
    preview: "MJPEG 预览占位",
    offline: "未连接",
    hint: "后续切片将在这里显示本地 WDA MJPEG 流。",
    command: "后端连通性",
    ready: "准备就绪",
    ping: "Ping 后端",
    result: "后端响应：",
    error: "调用失败：",
  },
  en: {
    title: "QuadControl console",
    subtitle: "Cross-platform desktop controller shell",
    preview: "MJPEG preview placeholder",
    offline: "Offline",
    hint: "A local WDA MJPEG stream will appear here in a later slice.",
    command: "Backend connectivity",
    ready: "Ready",
    ping: "Ping backend",
    result: "Backend response: ",
    error: "Call failed: ",
  },
};

const locale = navigator.language.toLowerCase().startsWith("zh") ? "zh" : "en";
const text = messages[locale];
document.documentElement.lang = locale === "zh" ? "zh-Hans" : "en";
document.querySelectorAll("[data-i18n]").forEach((element) => {
  element.textContent = text[element.dataset.i18n];
});

document.querySelector("#ping-button").addEventListener("click", async () => {
  const result = document.querySelector("#ping-result");
  result.textContent = "…";
  try {
    result.textContent = text.result + await window.__TAURI__.core.invoke("ping");
  } catch (error) {
    result.textContent = text.error + error;
  }
});
