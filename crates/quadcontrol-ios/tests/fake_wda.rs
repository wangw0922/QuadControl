//! 测试内的最小假 WDA HTTP 服务（手写 std TcpListener，不加 HTTP 依赖）。
//!
//! 只实现会话链路真正会调的那几个端点。刻意可编程：`send_text` 回读不一致、
//! `wake` 后仍锁定这两条用例，全靠在这里配置响应。

#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

/// 假 WDA 的行为开关。
#[derive(Debug, Clone, Default)]
pub struct FakeWdaConfig {
    /// `/wda/locked` 返回的值。
    pub locked: bool,
    /// 收到 `/wda/unlock` 之后是否解锁。false = 模拟「设了密码，唤醒无效」。
    pub unlock_succeeds: bool,
    /// `/wda/unlock` 是否直接回 500 且保持锁定。
    ///
    /// 这是**真机行为**：有密码的 iPhone 上该请求阻塞约 8 s 后返回 500
    /// `Timed out while waiting until the screen is unlocked`，而屏幕其实已经亮了、
    /// 停在密码页。`wake()` 必须把它判成 `device_locked`，而不是会话故障。
    pub unlock_errors: bool,
    /// `GET /element/active` 是否回 `nosuchelement`（设备上没有聚焦的输入框）。
    pub no_active_element: bool,
    /// 属性回读返回的内容；None = 回读发出去的文本（正常情况）。
    pub read_back_override: Option<String>,
    /// `POST /session` 是否失败。
    pub session_fails: bool,
    /// 窗口尺寸。
    pub window: (u32, u32),
}

impl FakeWdaConfig {
    pub fn healthy() -> Self {
        Self {
            locked: false,
            unlock_succeeds: true,
            unlock_errors: false,
            no_active_element: false,
            read_back_override: None,
            session_fails: false,
            window: (375, 667),
        }
    }
}

pub struct FakeWda {
    pub port: u16,
    locked: Arc<AtomicBool>,
    last_text: Arc<Mutex<String>>,
    pub requests: Arc<Mutex<Vec<String>>>,
    pub delete_session_calls: Arc<AtomicU64>,
}

impl FakeWda {
    pub fn start(config: FakeWdaConfig) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let locked = Arc::new(AtomicBool::new(config.locked));
        let last_text = Arc::new(Mutex::new(String::new()));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let delete_session_calls = Arc::new(AtomicU64::new(0));

        let state = State {
            config,
            locked: Arc::clone(&locked),
            last_text: Arc::clone(&last_text),
            requests: Arc::clone(&requests),
            delete_session_calls: Arc::clone(&delete_session_calls),
        };
        thread::spawn(move || {
            for incoming in listener.incoming() {
                let Ok(stream) = incoming else { return };
                let state = state.clone();
                thread::spawn(move || handle(stream, &state));
            }
        });

        Self {
            port,
            locked,
            last_text,
            requests,
            delete_session_calls,
        }
    }

    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn set_locked(&self, locked: bool) {
        self.locked.store(locked, Ordering::SeqCst);
    }

    pub fn saw(&self, needle: &str) -> bool {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .any(|line| line.contains(needle))
    }
}

#[derive(Clone)]
struct State {
    config: FakeWdaConfig,
    locked: Arc<AtomicBool>,
    last_text: Arc<Mutex<String>>,
    requests: Arc<Mutex<Vec<String>>>,
    delete_session_calls: Arc<AtomicU64>,
}

fn handle(mut stream: TcpStream, state: &State) {
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(clone) => clone,
        Err(_) => return,
    });
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).unwrap_or(0) == 0 {
        return;
    }
    let mut headers = HashMap::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_owned());
        }
    }
    let length: usize = headers
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut body = vec![0u8; length];
    if length > 0 && reader.read_exact(&mut body).is_err() {
        return;
    }
    let body = String::from_utf8_lossy(&body).into_owned();

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");
    state
        .requests
        .lock()
        .unwrap()
        .push(format!("{method} {path}"));

    // 红线自查：本库绝不应该调用 /wda/keys（真机上它返回成功但不送达）。
    assert!(
        !path.contains("/wda/keys"),
        "被测代码调用了禁用的 /wda/keys 端点"
    );

    let json = route(state, method, path, &body);
    let response = format!(
        "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        if json.0 { "200 OK" } else { "500 Internal Server Error" },
        json.1.len(),
        json.1
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

/// 返回 (是否 200, JSON 体)。
fn route(state: &State, method: &str, path: &str, body: &str) -> (bool, String) {
    if path == "/status" {
        return (true, r#"{"value":{"state":"success"}}"#.to_owned());
    }
    if method == "POST" && path == "/session" {
        if state.config.session_fails {
            return (
                false,
                r#"{"value":{"error":"session not created"}}"#.to_owned(),
            );
        }
        return (true, r#"{"sessionId":"FAKESESSION"}"#.to_owned());
    }
    if method == "DELETE" && path.starts_with("/session/") && path.matches('/').count() == 2 {
        state.delete_session_calls.fetch_add(1, Ordering::SeqCst);
        return (true, r#"{"value":null}"#.to_owned());
    }
    if path.ends_with("/appium/settings") {
        return (true, r#"{"value":{}}"#.to_owned());
    }
    if path.ends_with("/window/size") {
        let (w, h) = state.config.window;
        return (true, format!(r#"{{"value":{{"width":{w},"height":{h}}}}}"#));
    }
    if path.ends_with("/wda/locked") {
        let locked = state.locked.load(Ordering::SeqCst);
        return (true, format!(r#"{{"value":{locked}}}"#));
    }
    if path.ends_with("/wda/unlock") {
        if state.config.unlock_errors {
            // 照真机原文，包括 `unknown error` 这个 WDA 的笼统分类。锁定状态不变。
            return (
                false,
                r#"{"value":{"error":"unknown error","message":"Timed out while waiting until the screen is unlocked"}}"#
                    .to_owned(),
            );
        }
        if state.config.unlock_succeeds {
            state.locked.store(false, Ordering::SeqCst);
        }
        return (true, r#"{"value":null}"#.to_owned());
    }
    if path == "/wda/homescreen" || path.ends_with("/actions") {
        return (true, r#"{"value":null}"#.to_owned());
    }
    if path.ends_with("/element/active") {
        // **只有 GET 被处理**。真机（WDA 16.12.8）上 POST 返回
        // `unknown command / Unhandled endpoint`；替身照抄，这样一旦有人把
        // `send_text` 改回 POST，用例立刻红。
        if method != "GET" {
            return (
                false,
                r#"{"value":{"error":"unknown command","message":"Unhandled endpoint"}}"#
                    .to_owned(),
            );
        }
        if state.config.no_active_element {
            // 真机上没有聚焦输入框时的响应形状。
            return (
                false,
                r#"{"value":{"error":"nosuchelement","message":"Unable to find an element with focus"}}"#
                    .to_owned(),
            );
        }
        return (
            true,
            r#"{"value":{"element-6066-11e4-a52e-4f735466cecf":"E1"}}"#.to_owned(),
        );
    }
    if path.ends_with("/attribute/value") {
        let value = state
            .config
            .read_back_override
            .clone()
            .unwrap_or_else(|| state.last_text.lock().unwrap().clone());
        return (true, format!(r#"{{"value":"{value}"}}"#));
    }
    if path.ends_with("/value") {
        // {"value":["text"]} → 记下来供回读。
        if let Some(text) = body
            .split_once(r#""value":["#)
            .and_then(|(_, rest)| rest.split_once(']'))
            .map(|(inner, _)| inner.trim().trim_matches('"').to_owned())
        {
            *state.last_text.lock().unwrap() = text;
        }
        return (true, r#"{"value":null}"#.to_owned());
    }
    if path == "/screenshot" {
        // 合法的 base64 PNG magic。
        return (true, r#"{"value":"iVBORw0KGgo="}"#.to_owned());
    }
    (
        false,
        r#"{"value":{"error":"unknown endpoint"}}"#.to_owned(),
    )
}
