# macOS Bluetooth HID feasibility

M0 milestone item "macOS Bluetooth HID 能力检测". This records what a read-only
probe can establish, and — more importantly — what it cannot.

Run it with:

```sh
swift run QuadControlMacHIDProbe --format json
```

## The question

Controlling an iPhone from a Mac over Bluetooth would need the Mac to present
itself **as a keyboard** — the HID *peripheral* (device) role. macOS ships as a
HID *host*: it accepts keyboards and mice. These are opposite ends of the same
profile, and support for one says nothing about the other.

Taking the peripheral role requires three things:

1. publish an SDP record carrying the HID service class,
2. accept incoming L2CAP channels on PSM `0x11` (control) and `0x13` (interrupt),
3. withdraw the record afterwards so the Mac stops advertising the role.

## What the probe observed on the development host

macOS 26.5.1 (build 25F80), Apple BCM_4387 controller over PCIe:

- Controller present and powered; not discoverable.
- Controller advertises `HFP AVRCP A2DP HID Braille LEA AACP GATT SerialPort` —
  the HID profile is supported by the radio.
- `IOBluetooth.framework` loads.
- All three peripheral-role APIs exist in the Objective-C runtime and carry no
  deprecation attribute in the macOS 26.5 SDK:
  - `IOBluetoothSDPServiceRecord.publishedServiceRecordWithDictionary:`
  - `IOBluetoothSDPServiceRecord.removeServiceRecord`
  - `IOBluetoothL2CAPChannel.registerForChannelOpenNotifications:selector:withPSM:direction:`

## What stays unknown

`can_act_as_hid_peripheral` is reported as **`unknown`**, and the probe will
never report anything else. Symbol presence is necessary but nowhere near
sufficient. None of the following can be observed without an active test:

- whether an unsigned or ad-hoc-signed process may publish a HID SDP record,
- whether macOS surrenders the HID PSMs to a third-party process at all,
- whether an iPhone will offer to pair with the resulting advertisement,
- whether the pairing survives sleep, reconnect, and iOS version changes.

A controller that lists `HID` proves the radio speaks the profile. It does not
prove this process may take the peripheral role, and the probe deliberately
refuses to blur the two.

## What the probe does not do

It is passive, matching the ADB probe's contract. It runs exactly one read-only
command, `system_profiler SPBluetoothDataType -json`, with a 15-second timeout
and a 1 MiB output bound, and otherwise only asks the Objective-C runtime whether
classes and selectors exist. It publishes no SDP record, opens no channel, pairs
no device, and changes no Bluetooth state. It also omits the controller's
Bluetooth address: that is a stable hardware identifier this diagnostic does not
need.

## Active experiment — part 1 (done)

The first half of the active experiment has now run on the development host,
with the user present. State was restored afterwards: every published record was
removed, and Bluetooth was left `State: On` / `Discoverable: Off`, unchanged.

### The process must be a real app bundle launched by LaunchServices

Touching the SDP publishing API routes through
`IOBluetoothCoreBluetoothCoordinator`, which is gated by the Bluetooth TCC
service. A process that fails the gate is **killed with SIGABRT**, not given an
error to handle:

| Process shape | Result |
|---|---|
| Plain CLI, no usage description | `exit 134` — TCC crash |
| CLI with `NSBluetoothAlwaysUsageDescription` in an embedded `__TEXT,__info_plist` section | `exit 134` — the section is not honoured |
| `.app` bundle with the key, executable exec'd directly | `exit 134` |
| `.app` bundle with the key, launched via `open` (LaunchServices) | runs; TCC grants, `CBManager.authorization == 3` |

The crash reason is always the same string: the Info.plist must contain
`NSBluetoothAlwaysUsageDescription`. It is misleading — the key was present in
rows two and three. What actually matters is that LaunchServices launched the
bundle. Anything that needs Bluetooth must therefore ship as a bundled app, not
as the SwiftPM CLI the rest of this repo uses.

`CBManager.authorization` stays `notDetermined` until a `CBCentralManager` is
instantiated; that is what raises the prompt.

### The SDP dictionary format is not the one the headers suggest

A first attempt returned `nil` for every record and looked like a policy block.
It was not — the dictionary was malformed. Apple's own records, for example
`/System/Library/CoreServices/OBEXAgent.app/Contents/Resources/OBEXOPPSDPRecord.plist`,
show the real encoding:

- UUIDs are raw big-endian `Data` (`Data([0x11, 0x24])`), **not**
  `["DataElementType": 3, ...]`,
- `"0100 - ServiceName*"` is a plain `String`, not a data-element dictionary,
- integers use `["DataElementType": 1, "DataElementSize": n, "DataElementValue": …]`,
- a `LocalAttributes` dictionary carries `Persistent` and friends.

Treat a `nil` return as "check the dictionary against Apple's plists first". It
carries no error detail and looks identical to a refusal.

### What succeeded

With the correct format and Bluetooth authorized:

- HID service class `0x1124` published — handle `0x4f491124`, removed cleanly.
- SerialPort `0x1101` published as a control, also removed cleanly.
- `registerForChannelOpenNotifications:` succeeded for **both** HID PSMs,
  `0x0011` (control) and `0x0013` (interrupt).

So macOS does not reserve the HID service class or the HID PSMs against
third-party processes. The half of the question about local API access is
answered: **yes**.

## Direction dropped 2026-08-06 — part 2 never ran

Bluetooth HID was removed from the project before the second half of the
experiment ran. The reason is **loss of purpose, not a negative result**:
Mac→iPhone is now served by Apple's iPhone Mirroring and Windows→iPhone by
WebDriverAgent, so HID no longer serves any quadrant. See
[control architecture](CONTROL_ARCHITECTURE.md).

Nothing below was disproven. If both of those paths ever fail, HID is still an
open candidate and work resumes from exactly the three unknowns listed here.

## Active experiment — part 2 (never ran)

What is still unproven is the half that involves the phone:

1. The published record is a skeleton. For iOS to treat it as a keyboard it needs
   the full HID attribute set — report descriptor (`0x0206`), device subclass
   (`0x0202`), reconnect/virtual-cable flags, and the interrupt PSM in
   `AdditionalProtocolDescriptorList` (`0x000D`).
2. **Class of Device.** An iPhone filters what it offers to pair by CoD, and
   macOS advertises itself as `Computer`. No public API to change it was found.
   If it cannot be changed, a correct HID record may still never be offered as a
   keyboard — this is currently the most likely blocker.
3. **Discoverable state.** `IOBluetoothUserLib.h` exposes no public control for
   it.

Until an iPhone has actually paired and acted on a report, treat Mac→iPhone HID
control as **Blocked**. Whatever the outcome, this stays inside the project's
boundary: HID input is a user-authorized accessory, never a lock-screen bypass
and never unattended control.
