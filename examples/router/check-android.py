"""Exercise the installed router example on an Android device without clearing its data."""

import argparse
import json
import re
import shlex
import subprocess
import time
import xml.etree.ElementTree as ET

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--device", required=True, help="ADB serial, for example 192.168.0.109:5555")
args = parser.parse_args()
package = "dev.creamui.routerexample"
activity = f"{package}/android.app.NativeActivity"


def adb(*arguments):
    if arguments[0] == "shell":
        arguments = ("shell", shlex.join(arguments[1:]))
    result = subprocess.run(
        ["adb", "-s", args.device, *arguments],
        capture_output=True, text=True, timeout=30, check=True,
    )
    return result.stdout.strip()


def wait_for(check, description):
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        if check():
            return
        time.sleep(0.2)
    raise AssertionError(f"Timed out waiting for {description}")


def foreground():
    windows = adb("shell", "dumpsys", "window")
    return any(package in line for line in windows.splitlines() if "mCurrentFocus=" in line)


def history():
    # SharedPreferences.apply() can briefly expose an empty/missing XML file
    # while a route update is being committed. Retry that observation.
    for attempt in range(20):
        try:
            contents = adb("shell", "run-as", package, "cat", "shared_prefs/creamui-router.xml")
            value = ET.fromstring(contents).find("string[@name='main']")
            assert value is not None
            return json.loads(value.text)
        except (ET.ParseError, json.JSONDecodeError, subprocess.CalledProcessError):
            if attempt == 19:
                raise
            time.sleep(0.05)


def current():
    state = history()
    return state["entries"][state["index"]]


def expect(url):
    wait_for(lambda: current() == url, f"route {url}")
    assert foreground(), "The router example left the foreground"


def start(uri=None):
    command = ["am", "start", "-W", "-n", activity]
    if uri is not None:
        command += ["-a", "android.intent.action.VIEW", "-d", uri]
    adb("shell", *command)
    wait_for(foreground, "router Activity")
    # Wait for the first drawn frame before sending touch input.
    time.sleep(0.8)


def input_event(*command):
    assert foreground(), "Refusing to send input to another application"
    adb("shell", "input", *command)


dimensions = re.findall(r"size: (\d+)x(\d+)", adb("shell", "wm", "size"))[-1]
assert int(dimensions[0]) < int(dimensions[1]), "Run the sidebar fixture in portrait orientation"
adb("shell", "am", "force-stop", package)
start()
pid = adb("shell", "pidof", package)
original = history()
print(f"Started Android router; restored {current()}", flush=True)

density = adb("shell", "wm", "density")
scale = int(re.findall(r"density: (\d+)", density)[-1]) / 160
logs = adb("logcat", "-d", "-v", "brief", "--pid", pid, "creamui:I", "*:S")
insets = re.findall(r"system insets top=([\d.]+)", logs)
assert insets, "No window inset report; cannot locate sidebar safely"
top = float(insets[-1])


def tap(y):
    input_event("tap", str(round(80 * scale)), str(round((top + y) * scale)))


def route(index, url):
    tap(33 + 48 * index)
    expect(url)


route(0, "/")
route(1, "/users/42?tab=overview#details")
route(2, "/users/7?tab=settings")
input_event("keyevent", "KEYCODE_BACK")
expect("/users/42?tab=overview#details")
print("Sidebar params/query and system Back passed", flush=True)

tap(312)  # Forward.
expect("/users/7?tab=settings")
before = history()
tap(354)  # Replace with home.
expect("/")
after = history()
assert after["index"] == before["index"]
assert len(after["entries"]) == len(before["entries"])
tap(270)  # Back button.
expect("/users/42?tab=overview#details")
route(3, "/files/docs/router.md")
route(4, "/missing")
print("Forward, replace, wildcard and not-found routes passed", flush=True)

route(2, "/users/7?tab=settings")
saved = history()
adb("shell", "am", "force-stop", package)
start()
expect("/users/7?tab=settings")
assert history() == saved, "Process recreation changed the saved history"
assert adb("shell", "pidof", package) != pid, "Process was not recreated"
print("Complete history restoration after process recreation passed", flush=True)

adb("shell", "am", "force-stop", package)
start("https://router.creamui.test/users/a%2Fb?tab=deep&mode=test#fragment")
expect("/users/a%2Fb?tab=deep&mode=test#fragment")
print("Cold Intent deep link (encoded param, query and fragment) passed", flush=True)

while history()["index"] > 0:
    previous = history()["index"]
    input_event("keyevent", "KEYCODE_BACK")
    wait_for(lambda: history()["index"] == previous - 1, "Back traversal")
    assert foreground(), "System Back exited before reaching the history root"
input_event("keyevent", "KEYCODE_BACK")
wait_for(lambda: not foreground(), "Activity backgrounding at history root")
root_pid = adb("shell", "pidof", package)
print("System Back traversed history and backgrounded the task only at its root", flush=True)

start()
expect(original["entries"][0])
assert adb("shell", "pidof", package) == root_pid, "Reopening did not preserve the native process"
route(1, "/users/42?tab=overview#details")
input_event("keyevent", "KEYCODE_BACK")
expect(original["entries"][0])
print("Warm resume after root Back and subsequent navigation passed", flush=True)
print("Android router checks passed; example left open at the history root", flush=True)
