# Android device-side server (Mode B)

The plan's Mode B: the desktop pushes this over ADB for the session, runs it at
shell UID through `app_process`, and deletes it afterwards. It is never installed
and never persists.

```sh
./build.sh      # dex jar, no device needed
./run-probe.sh  # push, run, remove — cleanup runs even on failure
```

Not an APK. `app_process` needs no manifest, no resources, and no aapt2 — only
`android.jar` to compile against and `d8` to dex, so there is no Gradle here.

## Internal APIs

Mode B uses Android internal APIs through reflection. There is no public
equivalent for screen capture, input injection, or physical screen-off. See
[product constraints](../../../docs/PRODUCT_CONSTRAINTS.md) for the exception and
its bounds — most importantly that this is never store-distributed, runs only at
shell UID with the user's own ADB authorization, and is removed after the session.

Internal APIs move between Android versions and OEM builds, which is why the
first thing built here is a probe rather than a feature.

## Measured — Samsung SM-S9180, Android 16 (SDK 36)

| Capability | Finding |
|---|---|
| Screen capture | `SurfaceControl.getPhysicalDisplayIds()` **actually called**, returned 1 display |
| Input injection | `InputManager.getInstance()` → **NullPointerException**; `ServiceManager.getService("input")` → **binder obtained** |
| Physical screen-off | `SurfaceControl.setDisplayPowerMode` present, deliberately not invoked |

The input result is the one that matters. On Android 16 the widely-cited
`InputManager.getInstance()` route is dead — writing against it would crash on this
handset. The binder route through `ServiceManager`, which is what scrcpy uses, works.
Copying an approach from a tutorial and measuring it are different things.

## What the probe will not do

It reads state and exits. It captures no screen, injects no event, and changes no
setting. `injectInputEvent` and `setDisplayPowerMode` are looked up but never
invoked, because invoking them has a side effect on someone's phone — so
`verified_callable` stays false for both, and the JSON says why rather than
implying absence.

`class_present` and `method_present` are reported separately from
`verified_callable` on purpose. A symbol existing is not evidence the call works;
that distinction is what the input-injection result above turns on.

The device serial is not reported. The desktop already knows which device it
pushed to, and this project does not persist device identifiers.

## Not built yet

Screen capture, encoding, input injection, screen-off, and session restore. Those
are the next slices; this one exists to prove the delivery path — build, push,
launch, structured output, cleanup — and to measure the device before anything is
written against assumptions.
