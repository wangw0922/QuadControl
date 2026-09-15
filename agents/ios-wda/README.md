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
| Second `POST /session` from any client | **invalidates the previous session**: every session-scoped call on the old id returns 404 `invalid session id` (`Session … was deleted while this request was still pending`). WDA keeps exactly one session; the client must recreate on 404 |
| WKWebView `<img>` on a closed MJPEG stream | **never reconnects**; the last frame stays on screen with no event we could observe. The proxy must not drop a downstream connection |
| `ios` cwd side effect | go-ios writes `selfIdentity.plist` (pairing identity incl. a private key) into the **current working directory** of the process that runs it; now git-ignored, and the GUI must run go-ios with a controlled cwd before packaging (P7) |

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
