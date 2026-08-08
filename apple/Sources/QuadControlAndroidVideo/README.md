# Android mirroring and control — layer one

Live Android screen mirroring **and mouse/keyboard control** on macOS using
**only official `adb` commands**. No Android internal APIs, so it behaves the
same on any device with ADB debugging enabled.

```sh
swift run QuadControlAndroidViewer --max-size 1280 --bit-rate 8M
```

Close the window to stop; that also stops `screenrecord` on the device and the
persistent shells, which otherwise keep running.

| Gesture | Sent as |
|---|---|
| Click | `input tap` |
| Drag | one `input swipe` with the real duration |
| Scroll | `input swipe` opposite the wheel |
| Typing | `input text` |
| Backspace, Enter, Tab, arrows | named `input keyevent` |
| Esc | Back |
| Buttons under the picture | Back, Home, Recents, Volume |

## Input latency

Measured on a Samsung SM-S9180:

| | |
|---|---|
| `adb shell true` (round trip) | 38 ms |
| `adb shell input keyevent` (fresh `adb` each time) | 103 ms |
| **same, through one persistent shell** | **53 ms** |

So input goes through a single long-lived `adb shell`. The remaining ~51 ms is
the device starting a fresh `input` process per event, which layer one cannot
avoid — which is why a drag is one `input swipe` carrying its duration rather
than a stream of move events, and a string is one `input text`.

`input text` is ASCII-only: it maps characters onto key events, so anything else
is dropped by the device. Non-ASCII is filtered out and reported rather than
silently vanishing. **Chinese input does not work through layer one.**

## Rotation

`screenrecord` fixes its frame size at startup, so a rotation needs the stream
restarted — measured at **800–965 ms** before the first new frame. Rotation is
detected by polling `dumpsys window displays | grep -m1 cur=` every 0.35 s
(25 ms per poll, against 51–67 ms for the alternatives, and the only one that
reports the *rotated* size rather than the natural one).

`cur=` is load-bearing for correctness, not just for the window: `wm size` and
`init=` report the natural orientation, so using them would transpose every tap
whenever the phone is sideways.

To keep the 850 ms from reading as a freeze, the window is reshaped immediately
from the already-known new geometry and eased over 0.28 s about its centre, with
the previous frame left on screen meanwhile.

A square stream was tried, so a rotation would only change a crop and need no
restart at all. `screenrecord` does letterbox predictably — a 1280×1280 request
on a 3088×1440 display gave 1280×597 with 53% black rows — so it worked, but it
wastes about half the encoded area and leaves the window a shape that does not
match the phone. Rejected in favour of the animated restart.

## How it works

```
adb exec-out screenrecord --output-format=h264 --time-limit 0 -
   → AnnexBParser   splits the byte stream into NAL units
   → H264Decoder    VideoToolbox, hardware decode
   → DecodedFrame   CVPixelBuffer backed by IOSurface
   → window
```

`--time-limit 0` is load-bearing. The default is 180 seconds, after which the
picture simply freezes with no error.

## Measured

Samsung SM-S9180, Android 16, USB, on an M1 Pro:

| | |
|---|---|
| Stream | 1280×596 @ 25 fps, valid H.264 (ffprobe) |
| First byte from `screenrecord` | 0.37 s |
| Throughput | ~500 KB/s at 4 Mbps, continuous |
| Mirror window | 720×1600, sustained, 4.6% CPU |

**0.37 s is startup latency, not glass-to-glass.** The delay between something
happening on the phone and appearing in the window includes encode buffering,
transfer, decode, and draw, and has not been measured. Do not quote it as
interactive latency.

## Why the parser has to buffer

A pipe splits wherever it likes, including in the middle of a start code, so the
parser holds partial input across reads. It also emits a unit only once the *next*
start code arrives — handing a decoder a truncated trailing slice would corrupt
the picture. Tests cover every split position of a real stream prefix, byte-at-a-
time delivery, and both three- and four-byte start codes, because treating only
the four-byte form as a boundary silently merges two units into one bad slice.

The SPS/PPS in the tests were captured from this device. They are codec parameter
sets only — no picture data, so nothing from anyone's screen is stored.

## Not implemented

Input, clipboard, files, screen-off, audio. Layer one cannot do screen-off at
all; that needs the internal APIs described in
[control architecture](../../../docs/CONTROL_ARCHITECTURE.md).
