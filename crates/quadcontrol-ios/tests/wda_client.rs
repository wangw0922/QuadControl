//! WDA 客户端的效果核验测试：全部对着测试内的假 WDA 服务跑。

mod fake_wda;

use fake_wda::{FakeWda, FakeWdaConfig};
use quadcontrol_ios::wda::Client;

fn connected(config: FakeWdaConfig) -> (FakeWda, Client) {
    let server = FakeWda::start(config);
    let mut client = Client::with_base_url(server.base_url());
    client.create_session().unwrap();
    (server, client)
}

#[test]
fn status_and_session_and_settings_succeed_against_a_healthy_wda() {
    let (server, client) = connected(FakeWdaConfig::healthy());
    client.status().unwrap();
    assert_eq!(client.session_id(), Some("FAKESESSION"));
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
    let mut client = Client::with_base_url(server.base_url());
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
    assert!(server.saw("POST /session/FAKESESSION/element/active"));
    assert!(server.saw("POST /session/FAKESESSION/element/E1/value"));
    assert!(server.saw("GET /session/FAKESESSION/element/E1/attribute/value"));
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
