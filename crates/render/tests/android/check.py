import argparse
import io
import os
import subprocess
import time
from pathlib import Path

from PIL import Image

parser = argparse.ArgumentParser()
parser.add_argument("--device", default=os.environ.get("ANDROID_SERIAL", "emulator-5554"))
parser.add_argument("--backend", choices=["cpu", "gpu"], required=True)
parser.add_argument("--apk", type=Path, required=True)
args = parser.parse_args()
package = "dev.creamui.lifecycle"
activity = f"{package}/android.app.NativeActivity"


def adb(*arguments, binary=False):
    result = subprocess.run(
        ["adb", "-s", args.device, *arguments],
        check=True,
        capture_output=True,
        timeout=180 if arguments[0] == "install" else 30,
    )
    return result.stdout if binary else result.stdout.decode().strip()


def wait_for(check, description):
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        if check():
            return
        time.sleep(0.3)
    raise AssertionError(f"Timed out waiting for {description}")


def pixels_match(expected):
    screenshot = Image.open(io.BytesIO(adb("exec-out", "screencap", "-p", binary=True))).convert("RGB")
    width, height = screenshot.size
    points = [(width // 2, height // 4), (width // 2, height * 3 // 4)]
    for point, color in zip(points, [(200, 40, 60), expected]):
        if any(abs(actual - wanted) > 3 for actual, wanted in zip(screenshot.getpixel(point), color)):
            return False
    return True


rotation = adb("shell", "settings", "get", "system", "user_rotation")
automatic = adb("shell", "settings", "get", "system", "accelerometer_rotation")
try:
    adb("shell", "settings", "put", "system", "accelerometer_rotation", "0")
    adb("shell", "settings", "put", "system", "user_rotation", "0")
    adb("install", "--no-incremental", "-r", str(args.apk))
    adb("shell", "input", "keyevent", "KEYCODE_WAKEUP")
    adb("shell", "wm", "dismiss-keyguard")
    adb("shell", "am", "force-stop", package)
    adb("shell", "am", "start", "-W", "-n", activity)
    pid = adb("shell", "pidof", package)
    assert pid, "Application did not start"

    def logs():
        return adb("logcat", "-d", "-v", "brief", "--pid", pid, "creamui-lifecycle:D", "*:S")

    wait_for(lambda: f"{args.backend} presenter ready" in logs(), "requested presenter")
    wait_for(lambda: pixels_match((40, 60, 200)), "initial frame")
    screen = Image.open(io.BytesIO(adb("exec-out", "screencap", "-p", binary=True)))
    adb("shell", "input", "tap", str(screen.width // 2), str(screen.height * 3 // 4))
    wait_for(lambda: pixels_match((40, 200, 60)), "partial update")

    for cycle in range(1, 4):
        adb("shell", "input", "keyevent", "KEYCODE_HOME")
        wait_for(lambda: logs().count("window presenter suspended") >= cycle, "surface suspension")
        adb("shell", "am", "start", "-W", "-n", activity)
        wait_for(lambda: logs().count(f"{args.backend} presenter resumed") >= cycle, "presenter recreation")
        assert adb("shell", "pidof", package) == pid, "Application restarted during suspension"
        wait_for(lambda: pixels_match((40, 200, 60)), "retained application state")

    adb("shell", "settings", "put", "system", "user_rotation", "1")
    def landscape():
        image = Image.open(io.BytesIO(adb("exec-out", "screencap", "-p", binary=True)))
        return image.width > image.height

    wait_for(landscape, "landscape surface")
    wait_for(lambda: pixels_match((40, 200, 60)), "rotated frame")
    adb("shell", "settings", "put", "system", "user_rotation", "0")
    wait_for(lambda: not landscape(), "portrait surface")
    wait_for(lambda: pixels_match((40, 200, 60)), "portrait frame")
    assert adb("shell", "pidof", package) == pid, "Application restarted during rotation"
    screen = Image.open(io.BytesIO(adb("exec-out", "screencap", "-p", binary=True)))
    adb("shell", "input", "tap", str(screen.width // 2), str(screen.height * 3 // 4))
    wait_for(lambda: pixels_match((200, 160, 40)), "input after resume")
    output = logs()
    assert output.count("lifecycle-check: ready") == 1, output
    assert output.count("Android window request rejected") == 2, output
    assert "lifecycle-check: count=2" in output, output
    assert "panic:" not in output and "failed" not in output, output
    print(f"Android {args.backend} presentation, damage, suspension, rotation, state, input, and window rejection passed")
finally:
    adb("shell", "am", "force-stop", package)
    for key, value in [("user_rotation", rotation), ("accelerometer_rotation", automatic)]:
        if value == "null":
            adb("shell", "settings", "delete", "system", key)
        else:
            adb("shell", "settings", "put", "system", key, value)
