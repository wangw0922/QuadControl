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
    /// 第一次 session 作用域调用回 404 `invalid session id`，之后接受新会话。
    ///
    /// 模拟的是**真机行为**（2026-09-15）：另一个客户端 `POST /session` 之后，
    /// 原 session id 对任何 session 作用域端点都返回 404；WDA 同一时刻只留一个
    /// 会话，重启 WDA 同理。
    pub invalidate_after_first_call: bool,
    /// `POST /wda/apps/activate` 是否回 500（应用不存在之类的非锁定失败）。
    pub activate_fails: bool,
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
            invalidate_after_first_call: false,
            activate_fails: false,
            window: (375, 667),
        }
    }
}

pub struct FakeWda {
    pub port: u16,
    locked: Arc<AtomicBool>,
    last_text: Arc<Mutex<String>>,
    /// 当前**唯一**有效的 session id；别的 id 一律 404。None = 还没建会话。
    current_session: Arc<Mutex<Option<String>>>,
    /// 当前前台应用的 bundle id；`apps/activate` 改它，`activeAppInfo` 读它。
    active_bundle: Arc<Mutex<String>>,
    pub requests: Arc<Mutex<Vec<String>>>,
    /// 每个请求的 `(路径, 请求体)`；体里的内容也是契约的一部分
    /// （`pressButton` 的 `{"name":"home"}`）。
    bodies: Arc<Mutex<Vec<(String, String)>>>,
    pub delete_session_calls: Arc<AtomicU64>,
}

impl FakeWda {
    pub fn start(config: FakeWdaConfig) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let locked = Arc::new(AtomicBool::new(config.locked));
        let last_text = Arc::new(Mutex::new(String::new()));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let bodies = Arc::new(Mutex::new(Vec::new()));
        let delete_session_calls = Arc::new(AtomicU64::new(0));
        let current_session = Arc::new(Mutex::new(None));
        // 真机上 activeAppInfo 在主屏时回的就是 SpringBoard。
        let active_bundle = Arc::new(Mutex::new(SPRINGBOARD_BUNDLE_ID.to_owned()));

        let state = State {
            config,
            locked: Arc::clone(&locked),
            last_text: Arc::clone(&last_text),
            current_session: Arc::clone(&current_session),
            active_bundle: Arc::clone(&active_bundle),
            sessions_created: Arc::new(AtomicU64::new(0)),
            invalidated_once: Arc::new(AtomicBool::new(false)),
            requests: Arc::clone(&requests),
            bodies: Arc::clone(&bodies),
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
            current_session,
            active_bundle,
            requests,
            bodies,
            delete_session_calls,
        }
    }

    /// 当前前台应用的 bundle id。
    pub fn active_bundle(&self) -> String {
        self.active_bundle.lock().unwrap().clone()
    }

    /// 手动作废当前会话，模拟「另一个客户端抢走了 WDA 的唯一会话」。
    pub fn invalidate_session(&self) {
        *self.current_session.lock().unwrap() = Some("STOLENSESSION".to_owned());
    }

    /// 请求日志里与 `line` **完全相等**的条数（`POST /session` 用它，否则会把
    /// `POST /session/<id>/...` 也数进来）。
    pub fn count_exact(&self, line: &str) -> usize {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|seen| seen.as_str() == line)
            .count()
    }

    /// 请求日志里包含 `needle` 的条数。
    pub fn count(&self, needle: &str) -> usize {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|line| line.contains(needle))
            .count()
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

    /// 有没有一个请求，路径含 `path_needle` **且**请求体含 `body_needle`。
    /// 子串匹配就够，不解析 JSON。
    pub fn saw_body(&self, path_needle: &str, body_needle: &str) -> bool {
        self.bodies
            .lock()
            .unwrap()
            .iter()
            .any(|(path, body)| path.contains(path_needle) && body.contains(body_needle))
    }
}

#[derive(Clone)]
struct State {
    config: FakeWdaConfig,
    locked: Arc<AtomicBool>,
    last_text: Arc<Mutex<String>>,
    current_session: Arc<Mutex<Option<String>>>,
    active_bundle: Arc<Mutex<String>>,
    sessions_created: Arc<AtomicU64>,
    invalidated_once: Arc<AtomicBool>,
    requests: Arc<Mutex<Vec<String>>>,
    bodies: Arc<Mutex<Vec<(String, String)>>>,
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
    state
        .bodies
        .lock()
        .unwrap()
        .push((path.to_owned(), body.clone()));

    // 红线自查：本库绝不应该调用 /wda/keys（真机上它返回成功但不送达）。
    assert!(
        !path.contains("/wda/keys"),
        "被测代码调用了禁用的 /wda/keys 端点"
    );

    let (status, json) = route(state, method, path, &body);
    let reason = match status {
        200 => "200 OK",
        404 => "404 Not Found",
        _ => "500 Internal Server Error",
    };
    let response = format!(
        "HTTP/1.1 {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
        json.len(),
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

/// 返回 (HTTP 状态码, JSON 体)。
fn route(state: &State, method: &str, path: &str, body: &str) -> (u16, String) {
    if path == "/status" {
        return (200, r#"{"value":{"state":"success"}}"#.to_owned());
    }
    if method == "POST" && path == "/session" {
        if state.config.session_fails {
            return (
                500,
                r#"{"value":{"error":"session not created"}}"#.to_owned(),
            );
        }
        // 第一个会话保持历史上的 `FAKESESSION`，之后依次编号，这样用例能直接断言
        // 「客户端换到了哪个会话」。
        let nth = state.sessions_created.fetch_add(1, Ordering::SeqCst) + 1;
        let id = if nth == 1 {
            "FAKESESSION".to_owned()
        } else {
            format!("FAKESESSION-{nth}")
        };
        *state.current_session.lock().unwrap() = Some(id.clone());
        return (200, format!(r#"{{"sessionId":"{id}"}}"#));
    }
    if method == "DELETE" && path.starts_with("/session/") && path.matches('/').count() == 2 {
        state.delete_session_calls.fetch_add(1, Ordering::SeqCst);
        return (200, r#"{"value":null}"#.to_owned());
    }
    // session 作用域请求：id 必须是当前有效的那个，否则照真机回 404。
    if let Some(requested) = path.strip_prefix("/session/").and_then(|rest| {
        let id = rest.split('/').next().unwrap_or("");
        (!id.is_empty()).then(|| id.to_owned())
    }) {
        if state.config.invalidate_after_first_call
            && !state.invalidated_once.swap(true, Ordering::SeqCst)
        {
            // 「别人抢走了会话」：当前有效 id 变成别的，本次请求先吃一个 404。
            *state.current_session.lock().unwrap() = Some("STOLENSESSION".to_owned());
        }
        let current = state.current_session.lock().unwrap().clone();
        if current.as_deref() != Some(requested.as_str()) {
            return (404, INVALID_SESSION_BODY.to_owned());
        }
    }
    if path.ends_with("/appium/settings") {
        return (200, r#"{"value":{}}"#.to_owned());
    }
    if path.ends_with("/window/size") {
        let (w, h) = state.config.window;
        return (200, format!(r#"{{"value":{{"width":{w},"height":{h}}}}}"#));
    }
    if path.ends_with("/wda/locked") {
        let locked = state.locked.load(Ordering::SeqCst);
        return (200, format!(r#"{{"value":{locked}}}"#));
    }
    if path.ends_with("/wda/unlock") {
        if state.config.unlock_errors {
            // 照真机原文，包括 `unknown error` 这个 WDA 的笼统分类。锁定状态不变。
            return (
                500,
                r#"{"value":{"error":"unknown error","message":"Timed out while waiting until the screen is unlocked"}}"#
                    .to_owned(),
            );
        }
        if state.config.unlock_succeeds {
            state.locked.store(false, Ordering::SeqCst);
        }
        return (200, r#"{"value":null}"#.to_owned());
    }
    if path.ends_with("/wda/activeAppInfo") {
        let bundle = state.active_bundle.lock().unwrap().clone();
        // 真机的形状；`name` 常常是空串，客户端不该依赖它。
        return (
            200,
            format!(r#"{{"value":{{"bundleId":"{bundle}","pid":1234,"name":""}}}}"#),
        );
    }
    if path.ends_with("/wda/apps/activate") {
        if state.config.activate_fails {
            return (
                500,
                r#"{"value":{"error":"unknown error","message":"Cannot launch the application"}}"#
                    .to_owned(),
            );
        }
        // 与 `/value` 路由同样的粗切法：不为替身引入 JSON 依赖。
        // **只认单键请求体**（客户端只发 `{"bundleId":..}`）；多加一个键就切错了。
        if let Some(bundle) = body
            .split_once(r#""bundleId":"#)
            .and_then(|(_, rest)| rest.split_once('}'))
            .map(|(inner, _)| inner.trim().trim_matches('"').to_owned())
        {
            *state.active_bundle.lock().unwrap() = bundle;
        }
        return (200, r#"{"value":null}"#.to_owned());
    }
    if path.ends_with("/wda/dragfromtoforduration") {
        return (200, r#"{"value":null}"#.to_owned());
    }
    if path.ends_with("/wda/tap") {
        // 点按走会话作用域的 `wda/tap`，与 `pressButton` 同形。
        if !path.starts_with("/session/") {
            return (
                500,
                r#"{"value":{"error":"unknown command","message":"Unhandled endpoint"}}"#
                    .to_owned(),
            );
        }
        return (200, r#"{"value":null}"#.to_owned());
    }
    if path.ends_with("/actions") {
        // W3C `actions` 在真机上可用，但一次点按要 1.50 s（`wda/tap` 是 0.01 s）。
        // 替身直接拒绝它，这样一旦有人把 `tap()` 改回 `actions`，用例立刻红。
        // 响应体故意**不含** `invalid session`，且状态码不是 404，所以
        // `is_invalid_session` 不成立，不会触发会话重建。
        return (
            500,
            r#"{"value":{"error":"unknown command","message":"Unhandled endpoint"}}"#.to_owned(),
        );
    }
    if path.ends_with("/wda/pressButton") {
        // **只有会话作用域的路径被处理。** 真机（WDA 16.12.8）上顶层
        // `POST /wda/pressButton` 返回 `unknown command / Unhandled endpoint`，
        // 替身照抄，这样一旦有人把 `home()` 改回顶层路径，用例立刻红。
        if !path.starts_with("/session/") {
            return (
                500,
                r#"{"value":{"error":"unknown command","message":"Unhandled endpoint"}}"#
                    .to_owned(),
            );
        }
        return (200, r#"{"value":null}"#.to_owned());
    }
    if path.ends_with("/element/active") {
        // **只有 GET 被处理**。真机（WDA 16.12.8）上 POST 返回
        // `unknown command / Unhandled endpoint`；替身照抄，这样一旦有人把
        // `send_text` 改回 POST，用例立刻红。
        if method != "GET" {
            return (
                500,
                r#"{"value":{"error":"unknown command","message":"Unhandled endpoint"}}"#
                    .to_owned(),
            );
        }
        if state.config.no_active_element {
            // 真机上没有聚焦输入框时的响应形状。状态码按 W3C 用 404——这样
            // `is_invalid_session` 的「404 + invalid session」判据里，**只有** 404
            // 这一半成立，能证明客户端不会把任意 404 当成会话作废。
            return (
                404,
                r#"{"value":{"error":"nosuchelement","message":"Unable to find an element with focus"}}"#
                    .to_owned(),
            );
        }
        return (
            200,
            r#"{"value":{"element-6066-11e4-a52e-4f735466cecf":"E1"}}"#.to_owned(),
        );
    }
    if path.ends_with("/attribute/value") {
        let value = state
            .config
            .read_back_override
            .clone()
            .unwrap_or_else(|| state.last_text.lock().unwrap().clone());
        return (200, format!(r#"{{"value":"{value}"}}"#));
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
        return (200, r#"{"value":null}"#.to_owned());
    }
    if path == "/screenshot" {
        // 合法的 base64 PNG magic。
        return (200, r#"{"value":"iVBORw0KGgo="}"#.to_owned());
    }
    (500, r#"{"value":{"error":"unknown endpoint"}}"#.to_owned())
}

/// 主屏时 `activeAppInfo` 回的 bundle id。
pub const SPRINGBOARD_BUNDLE_ID: &str = "com.apple.springboard";

/// 真机 404 响应体的形状：WDA 把原因放在 `value.error` / `value.message` 里。
const INVALID_SESSION_BODY: &str =
    r#"{"value":{"error":"invalid session id","message":"Session does not exist"}}"#;
