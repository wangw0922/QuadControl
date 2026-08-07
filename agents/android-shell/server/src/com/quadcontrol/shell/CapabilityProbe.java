package com.quadcontrol.shell;

import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.List;

/**
 * Reports what this specific handset actually allows, from inside the device.
 *
 * <p>This runs through {@code app_process} at shell UID after the desktop pushes it
 * over ADB. It is not an installed app: there is no manifest, no Context, and no
 * package. It reads state and exits; it captures nothing, injects nothing, and
 * changes no device setting.
 *
 * <p>Mode B needs Android internal APIs — see {@code docs/PRODUCT_CONSTRAINTS.md} for
 * why that exception exists and what bounds it. Those APIs differ across versions and
 * OEM builds, so this probe exists to measure rather than assume.
 *
 * <p>The distinction that matters: a symbol being present is not the same as it being
 * callable. Each capability is therefore probed twice — reflection lookup, then a
 * genuinely side-effect-free invocation — and reported separately. Anything that
 * cannot be settled without causing a side effect stays {@code unknown}.
 */
public final class CapabilityProbe {

    private static final String VERSION = "1";

    public static void main(String[] args) {
        List<String> fields = new ArrayList<>();
        fields.add(str("probe_version", VERSION));
        // RELEASE lives on Build$VERSION, not Build.
        fields.add(str("android_release", versionField("RELEASE")));
        fields.add(num("android_sdk_int", sdkInt()));
        fields.add(str("manufacturer", buildField("MANUFACTURER")));
        fields.add(str("model", buildField("MODEL")));
        // The device serial is deliberately absent: the desktop already knows which
        // device it pushed to, and this project does not persist device identifiers.

        fields.add(obj("screen_capture", probeScreenCapture()));
        fields.add(obj("input_injection", probeInputInjection()));
        fields.add(obj("display_power", probeDisplayPower()));

        System.out.println("{" + join(fields) + "}");
    }

    /**
     * `SurfaceControl.getPhysicalDisplayIds` is read-only, so calling it proves the
     * class is genuinely reachable rather than merely present in the runtime.
     */
    private static String probeScreenCapture() {
        Capability capability = new Capability();
        capability.note = "SurfaceControl plus MediaCodec; MediaCodec itself is public API";
        try {
            Class<?> surfaceControl = Class.forName("android.view.SurfaceControl");
            capability.classPresent = true;

            Method ids = findMethod(surfaceControl, "getPhysicalDisplayIds");
            capability.methodPresent = ids != null;
            if (ids != null) {
                ids.setAccessible(true);
                Object result = ids.invoke(null);
                capability.callable = true;
                capability.detail = "physical display ids readable: " + describeLength(result);
            } else {
                capability.detail = "getPhysicalDisplayIds not found on this build";
            }
        } catch (Throwable error) {
            capability.detail = describe(error);
        }
        return capability.toJson();
    }

    /**
     * Injection itself has a side effect, so it is never attempted here. Reaching the
     * InputManager instance is the furthest this probe may safely go, which is why
     * `callable` stays false and the note says so.
     */
    private static String probeInputInjection() {
        Capability capability = new Capability();
        capability.note = "injectInputEvent is never invoked by this probe; it has a side effect";
        try {
            Class<?> inputManager = Class.forName("android.hardware.input.InputManager");
            capability.classPresent = true;

            Method inject = findMethod(inputManager, "injectInputEvent");
            capability.methodPresent = inject != null;

            // Two routes to a Context-free handle. `getInstance` is the obvious one and
            // is reported first; the ServiceManager binder route is what scrcpy uses and
            // is the fallback when the first fails, which it does on some builds.
            String viaGetInstance = tryGetInstance(inputManager);
            String viaServiceManager = tryServiceManager();
            capability.detail = "getInstance -> " + viaGetInstance
                    + "; ServiceManager -> " + viaServiceManager;
            capability.handleReachable = viaGetInstance.startsWith("ok")
                    || viaServiceManager.startsWith("ok");
        } catch (Throwable error) {
            capability.detail = describe(error);
        }
        return capability.toJson();
    }

    /**
     * Turning the panel off is exactly the side effect Mode B exists for, so it is
     * only looked up here, never invoked.
     */
    private static String probeDisplayPower() {
        Capability capability = new Capability();
        capability.note = "setDisplayPowerMode is never invoked by this probe; it turns the panel off";
        try {
            Class<?> surfaceControl = Class.forName("android.view.SurfaceControl");
            capability.classPresent = true;

            Method power = findMethod(surfaceControl, "setDisplayPowerMode");
            capability.methodPresent = power != null;
            capability.detail = power != null
                    ? "setDisplayPowerMode present; not invoked"
                    : "setDisplayPowerMode not found on this build";
        } catch (Throwable error) {
            capability.detail = describe(error);
        }
        return capability.toJson();
    }

    /** Reaching the service handle without a Context, without invoking anything. */
    private static String tryGetInstance(Class<?> inputManager) {
        try {
            Method instance = findMethod(inputManager, "getInstance");
            if (instance == null) {
                return "absent";
            }
            instance.setAccessible(true);
            return instance.invoke(null) != null ? "ok" : "returned null";
        } catch (Throwable error) {
            return describe(error);
        }
    }

    private static String tryServiceManager() {
        try {
            Class<?> serviceManager = Class.forName("android.os.ServiceManager");
            Method getService = findMethod(serviceManager, "getService");
            if (getService == null) {
                return "ServiceManager.getService absent";
            }
            getService.setAccessible(true);
            Object binder = getService.invoke(null, "input");
            return binder != null ? "ok, binder obtained" : "input service returned null";
        } catch (Throwable error) {
            return describe(error);
        }
    }

    private static final class Capability {
        boolean classPresent;
        boolean methodPresent;
        boolean callable;
        boolean handleReachable;
        String detail = "";
        String note = "";

        String toJson() {
            List<String> parts = new ArrayList<>();
            parts.add(bool("class_present", classPresent));
            parts.add(bool("method_present", methodPresent));
            parts.add(bool("verified_callable", callable));
            parts.add(bool("handle_reachable", handleReachable));
            parts.add(str("detail", detail));
            parts.add(str("note", note));
            return "{" + join(parts) + "}";
        }
    }

    private static Method findMethod(Class<?> owner, String name) {
        for (Method method : owner.getDeclaredMethods()) {
            if (method.getName().equals(name)) {
                return method;
            }
        }
        return null;
    }

    private static String describeLength(Object result) {
        if (result == null) {
            return "null";
        }
        if (result instanceof long[]) {
            return ((long[]) result).length + " entries";
        }
        if (result instanceof Object[]) {
            return ((Object[]) result).length + " entries";
        }
        return result.getClass().getSimpleName();
    }

    private static String describe(Throwable error) {
        Throwable cause = error.getCause() != null ? error.getCause() : error;
        String message = cause.getMessage();
        return cause.getClass().getName() + (message == null ? "" : ": " + message);
    }

    private static String buildField(String name) {
        try {
            return String.valueOf(Class.forName("android.os.Build").getField(name).get(null));
        } catch (Throwable error) {
            return "unknown";
        }
    }

    private static String versionField(String name) {
        try {
            return String.valueOf(
                    Class.forName("android.os.Build$VERSION").getField(name).get(null));
        } catch (Throwable error) {
            return "unknown";
        }
    }

    private static int sdkInt() {
        try {
            Class<?> version = Class.forName("android.os.Build$VERSION");
            return version.getField("SDK_INT").getInt(null);
        } catch (Throwable error) {
            return -1;
        }
    }

    private static String join(List<String> parts) {
        StringBuilder builder = new StringBuilder();
        for (int index = 0; index < parts.size(); index++) {
            if (index > 0) {
                builder.append(',');
            }
            builder.append(parts.get(index));
        }
        return builder.toString();
    }

    private static String str(String key, String value) {
        return "\"" + key + "\":\"" + escape(value) + "\"";
    }

    private static String num(String key, int value) {
        return "\"" + key + "\":" + value;
    }

    private static String bool(String key, boolean value) {
        return "\"" + key + "\":" + value;
    }

    private static String obj(String key, String rawJson) {
        return "\"" + key + "\":" + rawJson;
    }

    private static String escape(String value) {
        StringBuilder builder = new StringBuilder();
        for (int index = 0; index < value.length(); index++) {
            char character = value.charAt(index);
            switch (character) {
                case '"':
                    builder.append("\\\"");
                    break;
                case '\\':
                    builder.append("\\\\");
                    break;
                case '\n':
                    builder.append("\\n");
                    break;
                case '\r':
                    builder.append("\\r");
                    break;
                case '\t':
                    builder.append("\\t");
                    break;
                default:
                    if (character < 0x20) {
                        builder.append(String.format("\\u%04x", (int) character));
                    } else {
                        builder.append(character);
                    }
            }
        }
        return builder.toString();
    }

    private CapabilityProbe() {}
}
