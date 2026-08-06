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

## The active experiment this still needs

Closing the question requires a separate, explicitly user-approved step that
does mutate system state:

1. publish a HID SDP record and register for the two PSMs,
2. make the Mac discoverable and check whether an iPhone offers to pair,
3. if it pairs, send one HID report and observe whether iOS acts on it,
4. remove the record and restore the previous discoverable state.

That experiment needs a person present to accept the pairing prompt on the
phone, so it cannot be scripted. Until it runs, treat Mac→iPhone HID control as
**Blocked**, exactly like ReplayKit and Screen Curtain. Whatever the outcome,
this stays inside the project's boundary: HID input is a user-authorized
accessory, never a lock-screen bypass and never unattended control.
