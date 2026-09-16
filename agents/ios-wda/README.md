# Windows → iPhone agent boundary

This quadrant drives [go-ios](https://github.com/danielpaulus/go-ios) and
WebDriverAgent as external processes, the same way `agents/android-shell` drives
`adb`: a whitelisted, bounded subprocess with parsing separated from execution so
it can be tested without a device.

No client code is implemented yet. What follows is measured, not restated.

## Measured on hardware

Host: macOS 26.5.1 / Apple M1 Pro, go-ios v1.2.1, USB cable.
Device: iPhone SE 3 (iPhone14,6), iOS 26.5, Developer Mode on.

go-ios is cross-platform, so these results characterise the **device side**, which
Windows and macOS share. The Windows-specific tunnel driver and usbmuxd plumbing
are still unverified — that needs a Windows host.

| Capability | Result |
|---|---|
| `ios list`, `ios info` | works with **no tunnel**; 88 device fields |
| Tunnel for iOS 17+ services | `ios tunnel start --userspace` works **without root** |
| Screenshot, unlocked | 750×1334 PNG, real content, ~0.4 s |
| Screenshot throughput | 5 captures in 2.04 s ≈ **2.4 fps** |
| Build + sign WDA | works with a **free personal team** |
| Install WDA | succeeds **even while the phone is locked** |
| Run WDA | authorizes; HTTP API on device port 8100 via `ios forward` |
| Read window size | 375×667 points |
| Swipe / drag | `POST /session/{id}/wda/dragfromtoforduration` works |
| Tap | `POST /session/{id}/actions` (W3C) works; focus confirmed by keyboard |
| Home | top-level `POST /wda/homescreen` works |
| Text input | **only** `POST /session/{id}/element/{id}/value` works |

## Second measurement pass (2026-09-14, go-ios 1.2.1, WDA 16.12.8, iOS 26.5)

Same iPhone SE 3, driven through the `quadcontrol-ios` stack from a macOS host.
Numbers below supersede the August ones where they differ.

| Capability | Result |
|---|---|
| `ios tunnel start --userspace --udid=U` | info HTTP server up immediately; device tunnel `negotiated` **~1.1 s later**. Readiness = `GET /tunnels` lists the UDID, not "port is listening" |
| Tunnel discovery | without `--tunnel-info-port`, `runwda` / `forward` / `tunnel ls` attach to the **most recently registered** agent on the host, even on another port. A second tunnel process on the host therefore hijacks a session; pass the port explicitly to every subcommand |
| `ios runwda` → `GET /status` 200 | **1.6 s** |
| MJPEG (`mjpegServerFramerate` 15, quality 50) | 750×1334 JPEG, ~87 KB/frame, **41 frames in 3 s ≈ 13.7 fps** |
| MJPEG server handshake | pushes **nothing** until it receives an HTTP request line; header is `HTTP/1.0 200 OK`, `Content-Type: multipart/x-mixed-replace; boundary=--BoundaryString` (the boundary value itself carries `--`), parts use `Content-type: image/jpeg` + `Content-Length` |
| `element/active` | **`GET`** works; `POST` returns `unknown command / Unhandled endpoint` on this WDA build (the August note used POST) |
| setValue + read-back | `hello123` written to the Spotlight field, `attribute/value` read back identical |
| `wda/keys` | **delivered this time** (`zz` appended in Spotlight). Contradicts August; treat it as unreliable, not as broken. The client keeps using setValue |
| Locked screen, MJPEG | keeps streaming: 41 black frames in 3 s (~20 KB each). No frame-rate signal for lock state |
| Locked screen, `/screenshot` | 13 KB valid PNG, all black, no error |
| Locked screen, tap | returns success, no effect |
| `wda/unlock` on a passcode device | blocks **~8 s**, then HTTP 500 `Timed out while waiting until the screen is unlocked`; `/wda/locked` stays true. The screen wakes to the passcode page, which is the intended "wake" behaviour |
| `/wda/lock` | works; `/wda/locked` reports true within 1 s |
| `POST /wda/homescreen` from the second home page | returns success and does **nothing** (WDA treats SpringBoard as "already home"). Use `POST /session/{s}/wda/pressButton {"name":"home"}` to really press Home; the top-level `/wda/pressButton` is unhandled on this build |
| `POST /session/{s}/wda/dragfromtoforduration` | works (used for swipes) |
| `ios apps --list --udid=U` | one line per app: `<bundleId> <name…> <version>`; names may contain spaces (one of nine lines did), every line carried a version; some names carry an invisible leading U+200E |
| Tauri exit on macOS | AppleScript `quit` / Cmd+Q does **not** emit `RunEvent::ExitRequested`; only the final `RunEvent::Exit` fires. Cleanup hooked on `ExitRequested` alone left 4 orphaned go-ios children; hooking `Exit` (synchronous) reclaims them in ~3 s |
| Press-to-visible latency | from sending the `pressButton home` request to the first changed MJPEG frame **0.33 s** (the request itself blocks ~0.5 s on this device, so the frame arrived before the request returned); this is the mechanism floor, not bandwidth (37 KB × 23 fps ≈ 0.85 MB/s) |
| Tap latency, `POST /session/{s}/actions` (W3C) vs `POST /session/{s}/wda/tap` `{"x","y"}` (2026-09-15) | **1.50 s** vs **0.01 s** for one tap, same effect (verified repeatedly). The W3C path is what "feels slow"; the client now uses `wda/tap` |
| Other call latencies (2026-09-15) | `wda/dragfromtoforduration` **0.05 s**, `wda/locked` **0.04 s**, `window/size` **0.29 s** |
| MJPEG settings sweep (2026-09-15; frame sizes differ from the 2026-09-14 row above, likely different screen content, not verified — both sets are kept as measured) | `mjpegServerFramerate` 15 / quality 50 → **~14 fps, ~40 KB/frame**; framerate 30 / quality 30 → **~17 fps, ~37 KB/frame** (the client default); `mjpegScalingFactor` 50 gives **no frame-rate gain** — the bottleneck is device-side capture — so scaling is not used |
| Double-press Home for the app switcher (2026-09-15) | **not reachable.** Two `pressButton {"name":"home"}` calls, 150 ms apart and back to back, neither opens the app switcher: one XCUITest button press takes ~0.5 s, which misses the double-press window, and WDA exposes no public app-switcher endpoint |
| `GET /session/{s}/wda/apps/list` vs `activeAppInfo` (2026-09-15) | `apps/list` returns **only the foreground app**, so it cannot enumerate background apps. `activeAppInfo` answers in **0.24 s** with `{"value":{"bundleId":..,"pid":..,"name":..}}` (`name` is often empty) |
| `POST /session/{s}/wda/apps/activate` `{"bundleId":..}` (2026-09-15) | **works** — brought Chrome to the foreground. Together with `activeAppInfo` this replaces the unreachable app switcher: record the foreground apps seen during a session, then activate one |
| Second `POST /session` from any client | **invalidates the previous session**: every session-scoped call on the old id returns 404 `invalid session id` (`Session … was deleted while this request was still pending`). WDA keeps exactly one session; the client must recreate on 404 |
| WKWebView `<img>` on a closed MJPEG stream | **never reconnects**; the last frame stays on screen with no event we could observe. The proxy must not drop a downstream connection |
| `ios` cwd side effect | go-ios writes `selfIdentity.plist` (pairing identity incl. a private key) into the **current working directory** of the process that runs it; now git-ignored, and the GUI must run go-ios with a controlled cwd before packaging (P7) |

## Third measurement pass (2026-09-15/16): QuickTime USB screen stream vs WDA MJPEG

Question: can the picture path be replaced by the H.264 stream iOS offers over the
hidden "QuickTime" USB configuration (the one QuickTime Player and
`quicktime_video_hack` use), keeping control on WDA? Same iPhone SE 3, iOS 26.5,
macOS 26.5 host, go-ios 1.2.1.

| Step | Result |
|---|---|
| `quicktime_video_hack` (`explore/valeria-wifi-mirroring` branch, 2026-05, gst-free, MIT) `devices` / `activate` | both work: the phone gains a sixth USB configuration; macOS names its vendor interface "Valeria". Deactivation returns the active configuration to 5 but the extra configuration stays listed until replug (documented upstream) |
| `quicktime_video_hack record` on the macOS host | **blocked by the host**: `USBDeviceOpen: another process has device opened for exclusive access` (the `AppleUSBHostiOSDevice` kernel driver), then `libusb_claim_interface` on interface 2 fails with `IOCreatePlugInInterfaceForService: out of resources`, libusb -99. Same signature as upstream issue #159. Says nothing about Linux or Windows, where no such driver exists; both remain untested |
| Substitute meter: AVFoundation with `kCMIOHardwarePropertyAllowScreenCaptureDevices` | the phone appears as a muxed "iOS Device" (subtype `isr `). Camera permission is granted per launching app; a probe started from an agent shell is denied without a prompt, so the probe was run from Terminal.app |
| Stream, 100 s run, idle and swiping | **750×1334, 50.6 fps average, 53.9 fps inside motion windows**; arrival interval median **16.9 ms**, p99 53 ms, max 73 ms; **0 dropped frames**; host-arrival jitter (p99−p1 of wall−PTS) 61 ms. The frame rate stays at 40–57 fps on a static screen. Bitrate not measured: Apple's stack hands over decoded frames, not the H.264 bytes |
| Send-to-changed-frame latency | `pressButton home` **≤ 0.367 s** to the first frame whose luma differs (the request itself returned after 0.513 s, so the frame arrived before the XCTest call returned). Swipes: 1.44–1.87 s, while the drag request took 2.3–2.75 s. Both are dominated by XCTest executing the gesture; this is **parity** with the 0.33 s MJPEG figure, not an improvement, and the pure video latency cannot be split out without a device-side clock |
| Capture vs tunnel ordering | starting **or** stopping the capture re-enumerates USB: every live `ios tunnel start --userspace` connection failed at that instant and the WDA runner died with `lost connection to testmanagerd`. Rule: capture first, then tunnel and WDA; after the capture stops, restart both |
| Side effects on the device | the status bar shows Apple's demo values (9:41, full battery, no carrier) while captured; iOS 26 asked for the device passcode once ("验证以访问 XCTest") when WDA was relaunched after a long idle |
| `wda/tap` re-timed (2026-09-16, three fresh sessions, home screen and Settings, capture on and off) | **0.81–1.49 s per tap**, `wda/dragfromtoforduration` 1.7–3.1 s, `pressButton home` 0.51 s. The **0.01 s** in the row above was a **404 on an invalidated session** (a tap against a dead session id answers in ~25 ms; the GUI's probe session had just been invalidated by a second `POST /session`), not a real tap. `waitForIdleTimeout` 0, `animationCoolOffTimeout` 0, `defaultActiveApplication` set, `snapshotMaxDepth` 1 and `wda/touchAndHold` change nothing: the time is XCTest event synthesis itself. ~1 s per gesture is the control-side floor |
| Windows availability (desk research) | upstream gave up Windows; `chotgpt/quicktime_video_hack_windows` (C++ rewrite, MIT, commits to 2026-03) needs a replacement usbmuxd on port 37015 plus a libusb0 or filter driver that conflicts with Apple's driver |

Conclusion for engine selection: the stream is 2.5–3× the frame rate of the MJPEG path
at native resolution with no drops, and no worse in latency; the control path (WDA
gestures at ~1 s) is now the larger part of what a user feels as lag. Adopting the
stream is a Linux/Windows question that this macOS host cannot answer.

## Text input: use setValue, never wda/keys

`POST /session/{id}/wda/keys` **returns success and the characters never
arrive.** Verified twice: once with no focused element, and again with the field
properly focused (caret visible, keyboard up). The field stayed empty both times.

The device had the Simplified Chinese Pinyin IME active, which is the likely
cause and is the default configuration for this project's users — so treat
`wda/keys` as unusable rather than as an edge case.

Getting the active element and setting its value works reliably:

```
GET  /session/{id}/element/active        -> element id   (POST is unhandled on WDA 16.12.8)
POST /session/{id}/element/{id}/value    -> {"value":["192.0.2.7"]}
```

That put `192.0.2.7` into the field on the first try. A client must tap to
focus, read the active element, then setValue — and must verify the result rather
than trusting the return code.

## Three different silent-failure modes

This is the most important design constraint found so far. A client must not
decide it is "connected" from call success alone:

1. **Screenshot while locked** returns a perfectly valid 750×1334 PNG that is
   entirely black, with **no error**. A naive viewer shows a black screen and
   looks broken with nothing to diagnose.
2. **`/wda/keys` always returns success and never delivers text** on this device,
   focused or not. See the section above — use element `setValue` instead.
3. **Launching an app while locked** *does* fail loudly, with
   `FBSOpenApplicationErrorDomain Code=7 … Locked`.

So the same underlying condition — a locked phone — produces a silent black
frame, a silent no-op, and a loud error depending on which call you make. The
client has to detect each case and tell the user which one happened.

## Operational facts worth designing around

- **The phone auto-locks constantly.** Three separate test runs were interrupted
  by it. Any session flow needs to detect re-lock and prompt, not just fail.
- **2.4 fps is the per-screenshot ceiling.** Anything resembling smooth mirroring
  must use the MJPEG stream, and even then this number sets expectations well
  below scrcpy.
- **`ios ui …` is not the WDA you built.** Those subcommands talk to a separate
  agent on `127.0.0.1:12004`. Driving your own WDA means forwarding port 8100 and
  speaking its HTTP API directly.
- **WDA's default bundle id must be changed.** `com.facebook.WebDriverAgentRunner`
  cannot be signed by a personal team.

## Known limits, to be surfaced to users before install

- UI automation, not input injection. Games, continuous drags, multi-touch, and
  high frame rates will not match scrcpy.
- Setup requires trusting the computer, Developer Mode, a signed WebDriverAgent,
  and a tunnel.
- A free personal signing team expires after seven days.
- Cannot be distributed through the App Store.
- No screen-off control — XCUITest cannot drive a locked or dark display, and
  screenshots come back black.

## Rules this agent inherits

Public frameworks only, no jailbreak, no lock-screen bypass, no unattended
default. Every subprocess needs a timeout and an output bound. Device serial
numbers and addresses are not persisted.
