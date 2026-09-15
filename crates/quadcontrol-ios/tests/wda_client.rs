//! WDA 客户端的效果核验测试：全部对着测试内的假 WDA 服务跑。

mod fake_wda;

use fake_wda::{FakeWda, FakeWdaConfig};
use quadcontrol_ios::wda::Client;
use std::sync::{Arc, Barrier};
use std::thread;

fn connected(config: FakeWdaConfig) -> (FakeWda, Client) {
    let server = FakeWda::start(config);
    let client = Client::with_base_url(server.base_url());
    client.create_session().unwrap();
    (server, client)
}

#[test]
fn status_and_session_and_settings_succeed_against_a_healthy_wda() {
    let (server, client) = connected(FakeWdaConfig::healthy());
    client.status().unwrap();
    assert_eq!(client.session_id().as_deref(), Some("FAKESESSION"));
    client.configure_mjpeg(15, 50).unwrap();
    assert_eq!(client.window_size().unwrap().width, 375);
    assert!(server.saw("POST /session/FAKESESSION/appium/settings"));
}

#[test]
fn create_session_failure_maps_to_wda_session_failed() {
    let server = FakeWda::start(FakeWdaConfig {
        session_fails: true,
        ..FakeWdaConfig::healthy()
    });
    let client = Client::with_base_url(server.base_url());
    assert_eq!(
        client.create_session().unwrap_err().code(),
        "wda_session_failed"
    );
}

#[test]
fn tap_and_home_reach_the_documented_endpoints() {
    let (server, client) = connected(FakeWdaConfig::healthy());
    client.tap(120.0, 240.0).unwrap();
    client.home().unwrap();
    assert!(server.saw("POST /session/FAKESESSION/actions"));
    // 回主屏是**顶层**端点，不带 session 段。
    assert!(server.saw("POST /wda/homescreen"));
}

/// 锁定时点按不转发：真机上锁屏点按不报错也不生效（静默失败）。
#[test]
fn tap_is_refused_while_locked() {
    let (_server, client) = connected(FakeWdaConfig {
        locked: true,
        ..FakeWdaConfig::healthy()
    });
    assert_eq!(client.tap(1.0, 1.0).unwrap_err().code(), "device_locked");
}

/// 文字送达要**回读核验**，一致才算成功。
#[test]
fn send_text_succeeds_when_the_read_back_matches() {
    let (server, client) = connected(FakeWdaConfig::healthy());
    client.send_text("192.0.2.7").unwrap();
    // 取聚焦元素必须是 **GET**：真机（WDA 16.12.8）上 POST 回
    // `unknown command / Unhandled endpoint`，只有 GET 被处理。
    assert!(server.saw("GET /session/FAKESESSION/element/active"));
    assert!(!server.saw("POST /session/FAKESESSION/element/active"));
    assert!(server.saw("POST /session/FAKESESSION/element/E1/value"));
    assert!(server.saw("GET /session/FAKESESSION/element/E1/attribute/value"));
}

/// 设备上没有聚焦的输入框 → `no_active_element`，不是笼统的会话故障。
///
/// 真机（WDA 16.12.8）上 `GET /element/active` 此时回 `nosuchelement`。这是用户
/// 自己就能修的状态（点进一个输入框），值得一个明确的码和一句明确的提示。
#[test]
fn send_text_reports_no_active_element_when_nothing_is_focused() {
    let (_server, client) = connected(FakeWdaConfig {
        no_active_element: true,
        ..FakeWdaConfig::healthy()
    });
    let error = client.send_text("hello").unwrap_err();
    assert_eq!(error.code(), "no_active_element");
    assert!(
        error.to_string().contains("tap into a text field"),
        "{error}"
    );
}

/// 回读不含发送内容 → `text_unconfirmed`。
///
/// 安全输入框回读为空也会走到这里，属可接受的假阳性：宁可让用户去手机上核对，
/// 也不要谎称送达。
#[test]
fn send_text_reports_unconfirmed_when_the_read_back_differs() {
    for read_back in ["", "something else"] {
        let (_server, client) = connected(FakeWdaConfig {
            read_back_override: Some(read_back.to_owned()),
            ..FakeWdaConfig::healthy()
        });
        assert_eq!(
            client.send_text("192.0.2.7").unwrap_err().code(),
            "text_unconfirmed",
            "read_back={read_back:?}"
        );
    }
}

/// 空串没有可核验的效果，直接挡住，而不是走到一个让人困惑的 `text_unconfirmed`。
#[test]
fn send_text_rejects_an_empty_string() {
    let (server, client) = connected(FakeWdaConfig::healthy());
    assert_eq!(
        client.send_text("").unwrap_err().code(),
        "wda_session_failed"
    );
    // 一个请求都不该发出去。
    assert!(!server.saw("element/active"), "空串不该触碰设备");
}

/// 锁定时不转发文字（与 `tap` 一致）：锁屏下文字不会送达，而 WDA 不报错。
#[test]
fn send_text_is_refused_while_locked() {
    let (server, client) = connected(FakeWdaConfig {
        locked: true,
        ..FakeWdaConfig::healthy()
    });
    assert_eq!(
        client.send_text("hello").unwrap_err().code(),
        "device_locked"
    );
    // 锁定时连取聚焦元素都不该做。
    assert!(!server.saw("element/active"), "锁定时不该触碰设备");
}

/// 唤醒的效果核验 = 调前调后对比 `locked`。
#[test]
fn wake_unlocks_and_verifies_the_effect() {
    let (server, client) = connected(FakeWdaConfig {
        locked: true,
        ..FakeWdaConfig::healthy()
    });
    client.wake().unwrap();
    assert!(server.saw("POST /session/FAKESESSION/wda/unlock"));
    assert!(!client.locked().unwrap());
}

/// 唤醒后仍锁定（设了密码）→ `device_locked`，绝不尝试发送密码。
#[test]
fn wake_reports_device_locked_when_it_stays_locked() {
    let (server, client) = connected(FakeWdaConfig {
        locked: true,
        unlock_succeeds: false,
        ..FakeWdaConfig::healthy()
    });
    assert_eq!(client.wake().unwrap_err().code(), "device_locked");
    // 没有任何密码通路：请求里不该出现 passcode 之类的东西。
    let requests = server.requests.lock().unwrap().join(" ");
    assert!(!requests.to_lowercase().contains("passcode"), "{requests}");
}

/// `unlock` 返回 500 但屏幕已亮、仍锁定 → `device_locked`，**不是**会话故障。
///
/// 真机（iPhone SE 3 + WDA 16.12.8，有密码）：`POST /wda/unlock` 阻塞约 8 s 后
/// 返回 500 `Timed out while waiting until the screen is unlocked`，此后
/// `/wda/locked` 仍为 true——屏幕停在密码页，这正是我们要的唤醒效果。早先这条会
/// 被映射成 `wda_session_failed`，界面显示「无法建立会话」，是纯误报。
#[test]
fn wake_maps_an_erroring_unlock_to_device_locked() {
    let (server, client) = connected(FakeWdaConfig {
        locked: true,
        unlock_errors: true,
        ..FakeWdaConfig::healthy()
    });
    assert_eq!(client.wake().unwrap_err().code(), "device_locked");
    assert!(server.saw("wda/unlock"), "unlock 根本没发出去");
    // 仍然没有任何密码通路。
    let requests = server.requests.lock().unwrap().join(" ");
    assert!(!requests.to_lowercase().contains("passcode"), "{requests}");
}

/// 已经解锁时 `wake` 是无副作用的 no-op。
#[test]
fn wake_is_a_noop_when_already_unlocked() {
    let (server, client) = connected(FakeWdaConfig::healthy());
    client.wake().unwrap();
    assert!(!server.saw("wda/unlock"));
}

#[test]
fn screenshot_decodes_base64_into_png_bytes() {
    let (_server, client) = connected(FakeWdaConfig::healthy());
    let png = client.screenshot_png().unwrap();
    assert_eq!(&png[..4], &[0x89, 0x50, 0x4e, 0x47], "应当是 PNG magic");
}

#[test]
fn delete_session_is_best_effort_and_reaches_the_server() {
    let (server, client) = connected(FakeWdaConfig::healthy());
    client.delete_session().unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while server
        .delete_session_calls
        .load(std::sync::atomic::Ordering::SeqCst)
        == 0
    {
        assert!(
            std::time::Instant::now() < deadline,
            "DELETE /session 没到达"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// WDA 不可达时报 `wda_unreachable`，而不是挂住。
#[test]
fn unreachable_wda_reports_its_own_code() {
    let port = {
        let probe = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        probe.local_addr().unwrap().port()
    };
    let client = Client::new(port);
    assert_eq!(client.status().unwrap_err().code(), "wda_unreachable");
}

/// 会话被作废后，session 作用域调用要自行重建会话并重试，而不是把故障甩给用户。
///
/// 真机（2026-09-15）：另一个客户端 `POST /session` 之后，原 session id 对任何
/// session 作用域端点都返回 404 `invalid session id`，于是 `send_text` 在第一步
/// `locked()` 上就报 `wda_session_failed`——链路其实完好，用户却只能重开会话。
#[test]
fn session_scoped_calls_recreate_an_invalidated_session() {
    let server = FakeWda::start(FakeWdaConfig {
        invalidate_after_first_call: true,
        ..FakeWdaConfig::healthy()
    });
    let client = Client::with_base_url(server.base_url());
    client.create_session().unwrap();

    // 第一个 session 作用域调用撞上 404：必须**成功**，而不是报错。
    client.configure_mjpeg(15, 50).unwrap();

    assert_eq!(
        server.count_exact("POST /session"),
        2,
        "应当重建过一次会话：{:?}",
        server.requests.lock().unwrap()
    );
    assert_eq!(
        client.session_id().as_deref(),
        Some("FAKESESSION-2"),
        "客户端应当切到新会话"
    );
    // 新会话上必须重放 MJPEG 设置，否则帧率/画质退回 WDA 默认值。
    assert!(
        server.count("/session/FAKESESSION-2/appium/settings") >= 2,
        "重建后既要重放设置、又要重试原请求：{:?}",
        server.requests.lock().unwrap()
    );

    // 之后的调用照常走新会话。
    assert_eq!(client.window_size().unwrap().width, 375);
    assert!(server.saw("GET /session/FAKESESSION-2/window/size"));
}

/// 404 但不是「会话作废」的响应不得触发重建（否则任何 404 都会白建一个会话）。
///
/// 「没有聚焦元素」按 W3C 就是 404，响应体里没有 `invalid session`——正好把
/// `is_invalid_session` 的两半判据拆开验一次。
#[test]
fn an_unrelated_404_does_not_recreate_the_session() {
    let (server, client) = connected(FakeWdaConfig {
        no_active_element: true,
        ..FakeWdaConfig::healthy()
    });
    assert_eq!(
        client.send_text("hi").unwrap_err().code(),
        "no_active_element"
    );
    assert_eq!(server.count_exact("POST /session"), 1);
}

/// 两个克隆**同时**撞上作废，只能重建一次会话。
///
/// WDA 重启时就是这个形状：1 秒一次的状态轮询和用户命令各持一个克隆，同时吃 404。
/// 不串行化的话两边都会 `POST /session`，而 WDA 只留最后一个，先建的那条紧接着
/// 又 404，并且因为只重试一次而以 `wda_session_failed` 收场。
#[test]
fn concurrent_invalidations_recreate_the_session_only_once() {
    let server = FakeWda::start(FakeWdaConfig {
        invalidate_after_first_call: true,
        ..FakeWdaConfig::healthy()
    });
    let client = Client::with_base_url(server.base_url());
    client.create_session().unwrap();

    let barrier = Arc::new(Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let client = client.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                client.window_size()
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap().expect("并发重建不该让任何一边失败");
    }

    assert_eq!(
        server.count_exact("POST /session"),
        2,
        "只该重建一次会话（初次 + 重建）：{:?}",
        server.requests.lock().unwrap()
    );
}
