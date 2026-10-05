# Android presentation and lifecycle checks

This NativeActivity fixture verifies CPU and GPU pixels, partial frame damage,
three suspend/resume cycles, portrait/landscape resizing, retained reactive
state, input after resume, and rejection of extra windows and native popups.
The window-ready callback must run exactly once. The Python check preserves
rotation settings and requires Python 3 with Pillow, `adb`, and an unlocked
Android device or emulator with working GPU support.

Set `ANDROID_HOME`, `ANDROID_NDK_HOME`, and `JAVA_HOME`, and install
`cargo-apk` and the Rust target matching the device. For an x86_64 emulator,
run from the repository root:

```sh
rustup target add x86_64-linux-android
python3 -m pip install Pillow
CARGO_TARGET_DIR="$PWD/target" cargo apk build \
  --manifest-path crates/render/tests/android/Cargo.toml \
  --target x86_64-linux-android --features cpu
python3 crates/render/tests/android/check.py \
  --backend cpu --apk target/debug/apk/main.apk

CARGO_TARGET_DIR="$PWD/target" cargo apk build \
  --manifest-path crates/render/tests/android/Cargo.toml \
  --target x86_64-linux-android
python3 crates/render/tests/android/check.py \
  --backend gpu --apk target/debug/apk/main.apk
```

Use `--device SERIAL` or `ANDROID_SERIAL` to select a device. Use
`aarch64-linux-android` for an ARM64 device. The check installs the fixture as
`dev.creamui.lifecycle`, starts it, and returns to the launcher during each
suspension cycle. It changes system rotation settings temporarily and restores
them on exit. Its diagnostics use the `creamui-lifecycle` logcat tag.

```sh
adb logcat -s creamui-lifecycle
cargo fmt --manifest-path crates/render/tests/android/Cargo.toml --check
```
