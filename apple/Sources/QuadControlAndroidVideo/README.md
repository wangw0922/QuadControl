# Android mirroring — layer one

Live Android screen mirroring on macOS using **only official `adb` commands**. No
Android internal APIs, so it behaves the same on any device with ADB debugging
enabled.

```sh
swift run QuadControlAndroidViewer --width 720 --height 1600 --bit-rate 6M
```

Close the window to stop; that also stops `screenrecord` on the device, which
otherwise keeps running.

## How it works

```
adb exec-out screenrecord --output-format=h264 --time-limit 0 -
   → AnnexBParser   splits the byte stream into NAL units
   → H264Decoder    VideoToolbox, hardware decode
   → DecodedFrame   CGImage, converted off the main thread
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
