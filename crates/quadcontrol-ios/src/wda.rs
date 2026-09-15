//! WebDriverAgent 的阻塞式 HTTP 客户端。
//!
//! 每个请求都有超时（[`REQUEST_TIMEOUT`]）和响应上限（[`BODY_LIMIT`]）——红线
//! 要求「每个子进程/每次调用有超时与输出上限，不无界阻塞」。
//!
//! **结果码来自效果核验，不是 HTTP 200。** 真机测出 WDA 有三种静默失败，本模块
//! 对应地做了三处核验：
//! 1. [`Client::send_text`] 写完回读属性，不一致就报 [`Error::TextUnconfirmed`]；
//! 2. [`Client::wake`] 调前调后对比 [`Client::locked`]，仍锁定报 [`Error::DeviceLocked`]；
//! 3. 锁屏截图会返回**合法的全黑 PNG**，所以「停帧 / 黑屏」一律不当作锁屏信号，
//!    锁定提示只由 `/wda/locked` 驱动（本模块不做黑帧检测，代理也不解码 JPEG）。

use crate::Error;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 单次请求超时。
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
/// 响应体上限；截屏的 base64 PNG 是最大的一个响应。
pub const BODY_LIMIT: usize = 8 * 1024 * 1024;
/// `delete_session` 的超时：退出序列里它只是礼貌性调用，不能拖慢关闭预算。
pub const DELETE_SESSION_TIMEOUT: Duration = Duration::from_secs(1);
/// `wda/unlock` 的超时。
///
/// 必须远大于 [`REQUEST_TIMEOUT`]：真机实测有密码的设备上这个请求要**阻塞约 8 秒**
/// 才返回 500。用 5 s 的默认超时会把它判成传输失败，进而误报成会话故障。
pub const UNLOCK_TIMEOUT: Duration = Duration::from_secs(12);

/// 设备窗口尺寸，单位是**点**（不是像素）。真机 iPhone SE 3 是 375×667。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
}

/// WDA 客户端。克隆代价低（一个 URL、几个共享状态和 ureq 的 Agent），
/// 监督线程持有一份，GUI 命令层通过 [`crate::session::IosSessionHandle::client`]
/// 拿到克隆。
///
/// # 会话被作废时自愈
///
/// **真机实测（iPhone SE 3 + WDA 16.12.8，2026-09-15）**：另一个客户端对同一台
/// WDA `POST /session` 之后，原来的 session id 对**任何** session 作用域端点都返回
/// HTTP 404 `invalid session id`；WDA 同一时刻只保留一个会话，重启 WDA 也一样。
/// 当时的表现是 `send_text` 在第一步 `locked()` 上就以 `wda_session_failed` 失败，
/// 而链路其实完好——用户只能重开会话。
///
/// 所以每个 session 作用域请求撞上「404 + invalid session」时，会
/// [`Client::create_session`] 一次并**重试该请求一次**（只一次，避免打转），重建后
/// 重放记下的 MJPEG 设置。`session_id` 与 MJPEG 设置因此必须共享而不是各克隆一份：
/// 监督线程和 GUI 命令层看到的必须是同一个会话。
#[derive(Debug, Clone)]
pub struct Client {
    base_url: String,
    session_id: Arc<Mutex<Option<String>>>,
    /// 会话重建后要重放的 `(fps, quality)`；没配置过就是 None。
    mjpeg: Arc<Mutex<Option<(u32, u32)>>>,
    /// 把重建串行化，见 [`Client::recreate_session_unless_replaced`]。
    recreate_lock: Arc<Mutex<()>>,
    agent: ureq::Agent,
}

impl Client {
    /// 对着 `http://127.0.0.1:<port>` 建客户端。
    pub fn new(http_port: u16) -> Self {
        Self::with_base_url(format!("http://127.0.0.1:{http_port}"))
    }

    /// 对着任意 base URL 建客户端（测试用假 WDA 服务时走这条）。
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            session_id: Arc::new(Mutex::new(None)),
            mjpeg: Arc::new(Mutex::new(None)),
            recreate_lock: Arc::new(Mutex::new(())),
            agent: ureq::AgentBuilder::new().timeout(REQUEST_TIMEOUT).build(),
        }
    }

    /// 当前 session id；[`Client::create_session`] 之前是 None。
    ///
    /// 返回拥有所有权的 `String`：会话可能被后台重建，借出去的引用会立刻过期。
    pub fn session_id(&self) -> Option<String> {
        self.session_id
            .lock()
            .expect("session mutex poisoned")
            .clone()
    }

    fn session(&self) -> Result<String, Error> {
        self.session_id()
            .ok_or_else(|| Error::WdaSessionFailed("no WDA session has been created".into()))
    }

    /// 发一个 session 作用域请求；撞上「会话已被作废」就重建会话并**重试一次**。
    ///
    /// `call` 拿到的是当次要用的 session id，返回未包装的失败文案（判据要看原文）。
    /// 只重试一次：重建后还是 404 说明不是作废问题，再试下去只会打转。
    fn session_call<T>(
        &self,
        call: impl Fn(&str) -> Result<T, String>,
        wrap: impl Fn(String) -> Error,
    ) -> Result<T, Error> {
        let session = self.session()?;
        match call(&session) {
            Ok(value) => Ok(value),
            Err(message) if is_invalid_session(&message) => {
                self.recreate_session_unless_replaced(&session)?;
                call(&self.session()?).map_err(wrap)
            }
            Err(message) => Err(wrap(message)),
        }
    }

    /// 串行化的重建：只有当会话**仍是**那个作废的 `stale` 时才真的重建。
    ///
    /// WDA 重启时所有在飞的 session 调用会**同时**吃 404——1 秒一次的状态轮询
    /// （`window_size` + `locked`）和用户命令分别持有 `Client` 的克隆。不串行化的话
    /// 两边都会 `POST /session`，而 WDA 只留最后一个，先建的那条紧接着又 404；
    /// 因为只重试一次，它会以 `wda_session_failed` 收场——正是本机制要消灭的错误。
    ///
    /// 锁只跨「建会话 + 重放设置」两次 HTTP 调用，各自受 [`REQUEST_TIMEOUT`] 约束，
    /// 等待有界。
    fn recreate_session_unless_replaced(&self, stale: &str) -> Result<(), Error> {
        let _serial = self.recreate_lock.lock().expect("recreate mutex poisoned");
        if self.session()? != stale {
            // 别的克隆已经重建过了，直接拿新 id 去重试。
            return Ok(());
        }
        self.recreate_session()
    }

    /// 重建会话，并把记下的 MJPEG 设置重放上去。
    ///
    /// 重放用的是**不重试**的底层调用：重建路径上再触发一次重建就是递归。
    fn recreate_session(&self) -> Result<(), Error> {
        self.create_session()?;
        let settings = *self.mjpeg.lock().expect("mjpeg mutex poisoned");
        if let Some((fps, quality)) = settings {
            let session = self.session()?;
            self.configure_mjpeg_once(&session, fps, quality)
                .map_err(Error::WdaSessionFailed)?;
        }
        Ok(())
    }

    /// `GET /status`：就绪探测。起 WDA 后由调用方轮询。
    pub fn status(&self) -> Result<serde_json::Value, Error> {
        self.get("/status", Error::WdaUnreachable)
    }

    /// `POST /session`：建会话并记下 session id。
    ///
    /// 取 `&self`：session id 是共享状态，重建路径要能在克隆出去的客户端上调用。
    pub fn create_session(&self) -> Result<String, Error> {
        let body = serde_json::json!({ "capabilities": { "alwaysMatch": {} } });
        let value = self.post("/session", &body, Error::WdaSessionFailed)?;
        // WDA 把 sessionId 放在顶层或 value 里，两种形状都见过。
        let id = value
            .get("sessionId")
            .or_else(|| value.pointer("/value/sessionId"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| Error::WdaSessionFailed("WDA response has no sessionId".into()))?
            .to_owned();
        *self.session_id.lock().expect("session mutex poisoned") = Some(id.clone());
        Ok(id)
    }

    /// `POST /session/{s}/appium/settings`：限 MJPEG 帧率与画质。
    ///
    /// **这只作用在编码器侧**，与设备刷新率毫无关系——红线明令绝不修改设备
    /// 显示刷新率，本库没有也不会有任何刷新率参数。
    ///
    /// 参数会被记下来：会话若被作废并重建，这些设置要重放，否则新会话的 MJPEG
    /// 会退回 WDA 的默认帧率与画质。
    pub fn configure_mjpeg(&self, fps: u32, quality: u32) -> Result<(), Error> {
        *self.mjpeg.lock().expect("mjpeg mutex poisoned") = Some((fps, quality));
        self.session_call(
            |session| self.configure_mjpeg_once(session, fps, quality),
            Error::WdaSessionFailed,
        )
    }

    /// 单次 `appium/settings` 调用，不带重试；[`Client::recreate_session`] 也用它。
    fn configure_mjpeg_once(&self, session: &str, fps: u32, quality: u32) -> Result<(), String> {
        let path = format!("/session/{session}/appium/settings");
        let body = serde_json::json!({
            "settings": {
                "mjpegServerFramerate": fps,
                "mjpegServerScreenshotQuality": quality,
            }
        });
        self.post_raw(&path, &body).map(|_| ())
    }

    /// `GET /session/{s}/window/size`：旋转会改变它，调用方按状态轮询刷新。
    pub fn window_size(&self) -> Result<WindowSize, Error> {
        let value = self.session_call(
            |session| self.get_raw(&format!("/session/{session}/window/size")),
            Error::WdaSessionFailed,
        )?;
        let size = value.get("value").unwrap_or(&value);
        let field = |name: &str| {
            size.get(name)
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| Error::WdaSessionFailed(format!("window size has no {name}")))
        };
        Ok(WindowSize {
            width: field("width")? as u32,
            height: field("height")? as u32,
        })
    }

    /// `POST /session/{s}/actions`：W3C pointer 点按，坐标单位是**点**。
    ///
    /// 锁定时不转发：锁屏下点按不会产生效果，却不会报错（静默失败之一）。
    pub fn tap(&self, x_pt: f64, y_pt: f64) -> Result<(), Error> {
        if self.locked()? {
            return Err(Error::DeviceLocked);
        }
        let body = serde_json::json!({
            "actions": [{
                "type": "pointer",
                "id": "finger1",
                "parameters": { "pointerType": "touch" },
                "actions": [
                    { "type": "pointerMove", "duration": 0, "x": x_pt, "y": y_pt },
                    { "type": "pointerDown", "button": 0 },
                    { "type": "pause", "duration": 50 },
                    { "type": "pointerUp", "button": 0 },
                ],
            }],
        });
        self.session_call(
            |session| self.post_raw(&format!("/session/{session}/actions"), &body),
            Error::WdaSessionFailed,
        )
        .map(|_| ())
    }

    /// `POST /wda/homescreen`：回主屏。这是**顶层**端点，不带 session 段。
    pub fn home(&self) -> Result<(), Error> {
        self.post("/wda/homescreen", &serde_json::json!({}), |e| {
            Error::WdaSessionFailed(e)
        })?;
        Ok(())
    }

    /// `GET /session/{s}/wda/locked`：锁定状态。
    ///
    /// 这是**唯一**的锁定判据。停帧、黑屏都不是锁屏信号：真机测过，锁屏后截图
    /// 返回的是合法的全黑 PNG，没有任何错误。
    pub fn locked(&self) -> Result<bool, Error> {
        let value = self.session_call(
            |session| self.get_raw(&format!("/session/{session}/wda/locked")),
            Error::WdaSessionFailed,
        )?;
        value
            .get("value")
            .and_then(serde_json::Value::as_bool)
            .ok_or_else(|| Error::WdaSessionFailed("locked response has no boolean value".into()))
    }

    /// `POST /session/{s}/wda/unlock`：唤醒屏幕。
    ///
    /// 这是公开的 XCUITest 操作（Home + 上滑），**不是锁屏绕过**：设了密码的设备
    /// 会停在密码页，由用户自己在手机上解锁。本函数**绝不发送密码**，本库任何
    /// 地方都没有密码通路。
    ///
    /// 效果核验 = 调用前后对比 [`Client::locked`]。解锁有动画，立刻回读会假阴性，
    /// 所以做**有界**轮询（[`UNLOCK_SETTLE_BUDGET`]）；预算内仍锁定就报
    /// [`Error::DeviceLocked`]——那通常意味着设备有密码，需要用户介入。
    ///
    /// # 真机实测（iPhone SE 3 + WDA 16.12.8，有密码）
    ///
    /// `POST /wda/unlock` 会**阻塞约 8 秒**然后返回 HTTP 500：
    /// `Error Domain=com.facebook.WebDriverAgent Code=1 "Timed out while waiting
    /// until the screen is unlocked"`；此后 `/wda/locked` 仍为 true——屏幕已经点亮、
    /// 停在密码页，这**正是**我们要的「唤醒屏幕」效果。
    ///
    /// 两条由此而来的设计：
    /// 1. 这个请求单独用 [`UNLOCK_TIMEOUT`] 的 agent。用默认的 5 s
    ///    [`REQUEST_TIMEOUT`] 会先以 ureq 超时失败，被映射成 `wda_session_failed`，
    ///    界面显示成「无法建立会话」——对一台只是设了密码的手机来说是彻头彻尾的误报。
    /// 2. 请求失败（HTTP 500 或超时）**不立即报错**，而是回头查一次
    ///    [`Client::locked`]：仍锁定 → [`Error::DeviceLocked`]（预期结果，等用户
    ///    自己解锁）；已解锁 → `Ok`。只有 `locked()` 本身也失败，才是真的
    ///    [`Error::WdaSessionFailed`]。
    pub fn wake(&self) -> Result<(), Error> {
        if !self.locked()? {
            return Ok(());
        }
        let path = format!("/session/{}/wda/unlock", self.session()?);
        // 单独的 agent：unlock 在有密码的设备上要阻塞约 8 s 才返回 500。
        let agent = ureq::AgentBuilder::new().timeout(UNLOCK_TIMEOUT).build();
        let unlock = agent
            .post(&format!("{}{path}", self.base_url))
            .send_json(serde_json::json!({}));
        // unlock 不走 `session_call`：它本来就把「失败」交给 `locked()` 裁决，而
        // `locked()` 自己会在会话被作废时重建并重试，重建后下面那一轮轮询就落在
        // 新会话上。会话作废的情形里屏幕根本没被碰过，重发一次 unlock 没有意义。
        if unlock.is_err() {
            // 失败不代表没效果：屏幕通常已经亮了，只是停在密码页。让 `locked()`
            // 来裁决，而不是把一个预期内的 500 报成会话故障。
            return if self.locked()? {
                Err(Error::DeviceLocked)
            } else {
                Ok(())
            };
        }

        let deadline = std::time::Instant::now() + UNLOCK_SETTLE_BUDGET;
        loop {
            if !self.locked()? {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err(Error::DeviceLocked);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// `GET /screenshot`：返回解码后的 PNG 字节。
    ///
    /// **调用方必须知道**：锁屏时这里会返回一张完全合法、内容全黑的 PNG，没有
    /// 任何错误。要判断是不是锁屏，问 [`Client::locked`]，不要看像素。
    pub fn screenshot_png(&self) -> Result<Vec<u8>, Error> {
        let value = self.get("/screenshot", Error::ScreenshotFailed)?;
        let encoded = value
            .get("value")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| Error::ScreenshotFailed("screenshot response has no value".into()))?;
        decode_base64(encoded)
            .ok_or_else(|| Error::ScreenshotFailed("screenshot was not valid base64".into()))
    }

    /// 向当前聚焦元素送文字，并**回读核验**。
    ///
    /// 流程：`GET element/active` 取元素 → `POST element/{e}/value` 写入 →
    /// `GET element/{e}/attribute/value` 回读。回读不含发送内容就报
    /// [`Error::TextUnconfirmed`]，文案由 GUI 给（「无法确认已送达，请在手机上核对」）。
    ///
    /// **取元素必须用 GET。** 真机实测（WDA 16.12.8）：`POST
    /// /session/{s}/element/active` 返回
    /// `{"value":{"error":"unknown command","message":"Unhandled endpoint ..."}}`；
    /// 只有 `GET` 被处理（无聚焦元素时返回 `nosuchelement` 错误）。
    /// `agents/ios-wda/README.md` 里写的 POST 是旧版本 WDA 的行为。
    ///
    /// `nosuchelement` 映射成 [`Error::NoActiveElement`]，而不是笼统的会话故障：
    /// 「手机上没有聚焦的输入框」是用户能自己修的状态，值得一句明确的提示。
    ///
    /// **绝不调用 `POST /wda/keys`。** 真机验证两次（无聚焦元素、以及光标可见键盘
    /// 弹起的正常聚焦状态）：它**返回成功而字符从不送达**。设备当时启用了简体中文
    /// 拼音输入法，而那正是本项目用户的默认配置，所以 `wda/keys` 按「不可用」处理，
    /// 不是边缘情况。详见 `agents/ios-wda/README.md`。
    ///
    /// 安全输入框（密码框）回读为空是**正常**的，同样会走到 `TextUnconfirmed`——
    /// 这是可接受的假阳性：宁可让用户去手机上核对，也不要谎称送达。
    pub fn send_text(&self, text: &str) -> Result<(), Error> {
        // 空串没有可核验的效果：回读永远「不含」空串之外的东西，送不送都一样，
        // 只会走到一个让人困惑的 `TextUnconfirmed`。直接挡住。
        if text.is_empty() {
            return Err(Error::WdaSessionFailed("empty text".into()));
        }
        // 与 `tap` 一致：锁定时不转发。锁屏下文字不会送达，而 WDA 不会报错
        // （静默失败之一），照发只会得到一个假的「已发送」。
        if self.locked()? {
            return Err(Error::DeviceLocked);
        }
        // GET，不是 POST：见本函数的文档。WDA 对无聚焦元素回 `nosuchelement`，
        // 那不是故障，是「请先点进一个输入框」。
        let active = self.session_call(
            |session| self.get_raw(&format!("/session/{session}/element/active")),
            |message| {
                if message.contains("nosuchelement") {
                    Error::NoActiveElement
                } else {
                    Error::WdaSessionFailed(message)
                }
            },
        )?;
        let element = element_id(&active).ok_or(Error::NoActiveElement)?;
        // 元素 id 属于取到它的那个会话，所以写入与回读**不再重建会话**：真在这中间
        // 被作废了，重建后这个 id 已经没有意义，硬重试只会对着新会话的陌生元素写。
        // 那种情况如实报 `wda_session_failed`，用户重发一次就落在新会话上。
        let session = self.session()?;
        self.post(
            &format!("/session/{session}/element/{element}/value"),
            &serde_json::json!({ "value": [text] }),
            Error::WdaSessionFailed,
        )?;

        let read_back = self.get(
            &format!("/session/{session}/element/{element}/attribute/value"),
            Error::WdaSessionFailed,
        )?;
        let actual = read_back
            .get("value")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if actual.contains(text) {
            Ok(())
        } else {
            Err(Error::TextUnconfirmed)
        }
    }

    /// `DELETE /session/{s}`：退出序列里的礼貌性调用。
    ///
    /// 超时压到 [`DELETE_SESSION_TIMEOUT`]（≤ 1 s）：WDA 这时可能已经在死，
    /// 不能让它拖累关闭预算。失败一律忽略——终止梯子才是真正的回收手段。
    pub fn delete_session(&self) -> Result<(), Error> {
        let session = self.session()?;
        let agent = ureq::AgentBuilder::new()
            .timeout(DELETE_SESSION_TIMEOUT)
            .build();
        let _ = agent
            .delete(&format!("{}/session/{session}", self.base_url))
            .call();
        Ok(())
    }

    fn get(&self, path: &str, wrap: impl Fn(String) -> Error) -> Result<serde_json::Value, Error> {
        self.get_raw(path).map_err(wrap)
    }

    fn post(
        &self,
        path: &str,
        body: &serde_json::Value,
        wrap: impl Fn(String) -> Error,
    ) -> Result<serde_json::Value, Error> {
        self.post_raw(path, body).map_err(wrap)
    }

    /// 未包装的 GET：失败文案原样返回，好让 [`is_invalid_session`] 看原文判断。
    fn get_raw(&self, path: &str) -> Result<serde_json::Value, String> {
        let response = self
            .agent
            .get(&format!("{}{path}", self.base_url))
            .call()
            .map_err(|e| describe(path, e))?;
        read_json_raw(response, path)
    }

    /// 未包装的 POST；见 [`Client::get_raw`]。
    fn post_raw(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value, String> {
        let response = self
            .agent
            .post(&format!("{}{path}", self.base_url))
            .send_json(body.clone())
            .map_err(|e| describe(path, e))?;
        read_json_raw(response, path)
    }
}

/// 这条失败文案是不是「会话已被作废」。
///
/// 真机上的形状是 HTTP 404 + 响应体里的 `invalid session id`；WDA 各版本也写过
/// `Session does not exist`，两种都认。必须同时看 404：别的端点的响应体里出现
/// 同样的字样时不该触发重建。
fn is_invalid_session(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("returned 404")
        && (lower.contains("invalid session") || lower.contains("session does not exist"))
}

/// 解锁动画的有界等待预算；见 [`Client::wake`]。
pub const UNLOCK_SETTLE_BUDGET: Duration = Duration::from_secs(2);

/// 从 `element/active` 的响应里取元素 id。
///
/// WDA 会用旧的 `ELEMENT` 键或 W3C 的 `element-6066-11e4-a52e-4f735466cecf` 键，
/// 版本之间不一致，两种都认。
fn element_id(value: &serde_json::Value) -> Option<String> {
    let holder = value.get("value").unwrap_or(value);
    for key in ["ELEMENT", "element-6066-11e4-a52e-4f735466cecf"] {
        if let Some(id) = holder.get(key).and_then(serde_json::Value::as_str) {
            return Some(id.to_owned());
        }
    }
    None
}

/// 把 ureq 的错误转成带上下文的文案。
///
/// ureq 2.x 对 4xx/5xx 返回 `Error::Status(code, response)`，而 WDA 恰恰把失败
/// 原因放在那个响应体里，所以要把它读出来，不能只报一个状态码。
fn describe(path: &str, error: ureq::Error) -> String {
    match error {
        ureq::Error::Status(code, response) => {
            let mut body = Vec::new();
            let _ = response
                .into_reader()
                .take(BODY_LIMIT as u64)
                .read_to_end(&mut body);
            let text = String::from_utf8_lossy(&body);
            format!("WDA {path} returned {code}: {}", text.trim())
        }
        ureq::Error::Transport(transport) => format!("WDA {path} is unreachable: {transport}"),
    }
}

/// 读响应体并解析 JSON，带 [`BODY_LIMIT`] 上限。
///
/// 多读一个字节来判断是否**超**限：`take(n)` 读满 n 字节时无法区分「正好 n」和
/// 「被截断」，而截断后的 JSON 解析失败会报成一个误导性的语法错误。
fn read_json_raw(response: ureq::Response, path: &str) -> Result<serde_json::Value, String> {
    let mut body = Vec::new();
    response
        .into_reader()
        .take(BODY_LIMIT as u64 + 1)
        .read_to_end(&mut body)
        .map_err(|e| format!("WDA {path} body could not be read: {e}"))?;
    if body.len() > BODY_LIMIT {
        return Err(format!(
            "WDA {path} response exceeded the {BODY_LIMIT} byte limit"
        ));
    }
    if body.is_empty() {
        return Ok(serde_json::Value::Null);
    }
    serde_json::from_slice(&body).map_err(|e| format!("WDA {path} returned unparsable JSON: {e}"))
}

/// 标准 base64 解码（RFC 4648，带 `=` 填充）。
///
/// 手写而不是加一个 crate：整个仓库只有截屏这一处用得到 base64，解码器本身
/// 30 行、可测，比多一条供应链依赖划算。非法字符、非法长度一律返回 None。
/// 空白（WDA 有时会在 base64 里插换行）被跳过。
fn decode_base64(input: &str) -> Option<Vec<u8>> {
    fn sextet(c: u8) -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => u32::from(c - b'A'),
            b'a'..=b'z' => u32::from(c - b'a') + 26,
            b'0'..=b'9' => u32::from(c - b'0') + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        })
    }

    let cleaned: Vec<u8> = input.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    let (payload, padding) = match cleaned.iter().position(|&b| b == b'=') {
        Some(at) => {
            // 填充只能出现在末尾，且最多两个。
            if cleaned[at..].iter().any(|&b| b != b'=') || cleaned.len() - at > 2 {
                return None;
            }
            (&cleaned[..at], cleaned.len() - at)
        }
        None => (&cleaned[..], 0),
    };
    if (payload.len() + padding) % 4 != 0 {
        return None;
    }

    let mut out = Vec::with_capacity(payload.len() / 4 * 3);
    let mut accumulator = 0u32;
    let mut bits = 0u32;
    for &byte in payload {
        accumulator = (accumulator << 6) | sextet(byte)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((accumulator >> bits) as u8);
        }
    }
    // 残余位必须是填充产生的零位；否则输入被截断过。
    if accumulator & ((1 << bits) - 1) != 0 {
        return None;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips_known_vectors() {
        // RFC 4648 测试向量，覆盖三种填充长度。
        for (encoded, decoded) in [
            ("", ""),
            ("Zg==", "f"),
            ("Zm8=", "fo"),
            ("Zm9v", "foo"),
            ("Zm9vYg==", "foob"),
            ("Zm9vYmE=", "fooba"),
            ("Zm9vYmFy", "foobar"),
        ] {
            assert_eq!(
                decode_base64(encoded).unwrap(),
                decoded.as_bytes(),
                "{encoded}"
            );
        }
        // PNG magic：截屏路径真正要解出来的东西。
        assert_eq!(
            decode_base64("iVBORw0KGgo=").unwrap(),
            [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]
        );
        // 内嵌空白要能跳过。
        assert_eq!(decode_base64("Zm9v\nYmFy").unwrap(), b"foobar");
    }

    #[test]
    fn base64_rejects_malformed_input() {
        assert!(decode_base64("Zm9vYg=").is_none(), "长度不是 4 的倍数");
        assert!(decode_base64("Zm9*").is_none(), "非法字符");
        assert!(decode_base64("Z=m8").is_none(), "填充不在末尾");
        assert!(decode_base64("Zg===").is_none(), "填充过长");
    }

    #[test]
    fn element_id_accepts_both_legacy_and_w3c_keys() {
        let legacy = serde_json::json!({ "value": { "ELEMENT": "42" } });
        let w3c = serde_json::json!({ "value": { "element-6066-11e4-a52e-4f735466cecf": "abc" } });
        assert_eq!(element_id(&legacy).unwrap(), "42");
        assert_eq!(element_id(&w3c).unwrap(), "abc");
        assert!(element_id(&serde_json::json!({ "value": {} })).is_none());
    }

    /// 没建会话就调带 session 的方法，必须响亮失败，而不是拼出 `/session//…`。
    #[test]
    fn session_scoped_calls_require_a_session() {
        let client = Client::new(1);
        assert_eq!(
            client.window_size().unwrap_err().code(),
            "wda_session_failed"
        );
        assert!(client.session_id().is_none());
    }
}
